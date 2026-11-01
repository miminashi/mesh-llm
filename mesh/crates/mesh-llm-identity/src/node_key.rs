use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::CryptoError;

pub const NODE_KEY_BYTES: usize = 32;

/// Resolve the default node key path, honoring the explicit override.
///
/// `MESH_LLM_NODE_KEY_PATH` selects a dedicated key file for hosts running
/// several node processes side by side (#1699). When unset, the key stays at
/// `~/.mesh-llm/key` so existing installations keep their node identity.
pub fn default_node_key_path() -> Result<PathBuf, CryptoError> {
    resolve_node_key_path(std::env::var_os("MESH_LLM_NODE_KEY_PATH"))
}

/// Resolve a node key path with explicit inputs so precedence is testable
/// without mutating process environment variables.
///
/// An empty override is treated as unset, so `MESH_LLM_NODE_KEY_PATH=` keeps
/// the historical default rather than resolving to the working directory.
pub fn resolve_node_key_path(
    override_path: Option<std::ffi::OsString>,
) -> Result<PathBuf, CryptoError> {
    if let Some(path) = override_path
        && !path.is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    home_node_key_path()
}

/// The historical node key location: `~/.mesh-llm/key`.
pub fn home_node_key_path() -> Result<PathBuf, CryptoError> {
    let home = dirs::home_dir().ok_or_else(|| {
        CryptoError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine home directory",
        ))
    })?;
    Ok(home.join(".mesh-llm").join("key"))
}

pub fn load_node_key_bytes_from_path(path: &Path) -> Result<[u8; NODE_KEY_BYTES], CryptoError> {
    // Read through the descriptor that passed the checks, so that swapping
    // the path for a symlink afterwards cannot make us read another file.
    let mut file = open_private_node_key_file(path)?;
    let mut hex = String::new();
    file.read_to_string(&mut hex)?;
    let bytes = hex::decode(hex.trim()).map_err(|err| CryptoError::InvalidKeyMaterial {
        reason: format!("bad node key hex in {}: {err}", path.display()),
    })?;
    bytes
        .try_into()
        .map_err(|_| CryptoError::InvalidKeyMaterial {
            reason: format!(
                "node key in {} must be {NODE_KEY_BYTES} bytes",
                path.display()
            ),
        })
}

pub fn save_node_key_bytes_to_path(
    path: &Path,
    key_bytes: &[u8; NODE_KEY_BYTES],
) -> Result<(), CryptoError> {
    let parent = path.parent().ok_or_else(|| {
        CryptoError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("node key path {} has no parent directory", path.display()),
        ))
    })?;
    ensure_private_node_key_dir(parent)?;
    if path.exists() {
        open_private_node_key_file(path)?;
    }
    write_bytes_atomically(path, hex::encode(key_bytes).as_bytes())?;
    open_private_node_key_file(path)?;
    Ok(())
}

fn ensure_private_node_key_dir(dir: &Path) -> Result<(), CryptoError> {
    let mut missing = Vec::new();
    let mut current = dir;
    loop {
        // A relative path such as `keys/node.key` bottoms out at the empty
        // path, which is the current directory and already exists.
        if current.as_os_str().is_empty() {
            break;
        }
        match std::fs::metadata(current) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(CryptoError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        format!("node key parent {} is not a directory", current.display()),
                    )));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current);
                current = current.parent().ok_or_else(|| {
                    CryptoError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("cannot find parent directory for {}", dir.display()),
                    ))
                })?;
            }
            Err(error) => return Err(error.into()),
        }
    }

    for path in missing.into_iter().rev() {
        match create_private_node_key_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = std::fs::metadata(path)?;
                if !metadata.is_dir() {
                    return Err(CryptoError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        format!("node key parent {} is not a directory", path.display()),
                    )));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_node_key_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_node_key_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

fn not_a_regular_file(path: &Path) -> CryptoError {
    CryptoError::InvalidKeyMaterial {
        reason: format!("node key path {} is not a regular file", path.display()),
    }
}

/// Open the node key, check that it is a regular file, and make it private.
///
/// The type check, the permission repair and the caller's read all go
/// through the returned descriptor. `O_NOFOLLOW` refuses a symlink at the
/// final component and `O_NONBLOCK` keeps a FIFO from stalling the open, so
/// swapping the path after the checks cannot redirect the chmod or the read
/// to another file.
#[cfg(unix)]
fn open_private_node_key_file(path: &Path) -> Result<std::fs::File, CryptoError> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| {
            if error.raw_os_error() == Some(libc::ELOOP) {
                not_a_regular_file(path)
            } else {
                CryptoError::Io(error)
            }
        })?;
    #[cfg(test)]
    tests::run_after_open_hook();
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(not_a_regular_file(path));
    }
    let mut perms = metadata.permissions();
    if perms.mode() & 0o077 != 0 {
        perms.set_mode(0o600);
        file.set_permissions(perms)?;
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_private_node_key_file(path: &Path) -> Result<std::fs::File, CryptoError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(not_a_regular_file(path));
    }
    Ok(std::fs::File::open(path)?)
}

fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Result<(), CryptoError> {
    let parent = path.parent().ok_or_else(|| {
        CryptoError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("node key path {} has no parent directory", path.display()),
        ))
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        CryptoError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("node key path {} has no file name", path.display()),
        ))
    })?;
    let tmp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name.to_string_lossy(),
        std::process::id(),
        rand::random::<u64>()
    ));

    let write_result = (|| -> Result<(), CryptoError> {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }

        let mut file = options.open(&tmp_path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        std::fs::rename(&tmp_path, path)?;

        #[cfg(unix)]
        {
            let dir = std::fs::File::open(parent)?;
            dir.sync_all()?;
        }

        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }

    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    type AfterOpenHook = Box<dyn FnOnce()>;

    thread_local! {
        static AFTER_OPEN_HOOK: std::cell::RefCell<Option<AfterOpenHook>> =
            const { std::cell::RefCell::new(None) };
    }

    /// Run `hook` once, on this thread, right after the node key is opened.
    #[cfg(unix)]
    fn set_after_open_hook(hook: impl FnOnce() + 'static) {
        AFTER_OPEN_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    }

    pub(super) fn run_after_open_hook() {
        if let Some(hook) = AFTER_OPEN_HOOK.with(|slot| slot.borrow_mut().take()) {
            hook();
        }
    }

    fn temp_node_key_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mesh-node-key-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("key")
    }

    #[test]
    fn node_key_bytes_round_trip() {
        let path = temp_node_key_path();
        let key = [7u8; NODE_KEY_BYTES];

        save_node_key_bytes_to_path(&path, &key).unwrap();

        assert_eq!(load_node_key_bytes_from_path(&path).unwrap(), key);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[cfg(unix)]
    #[test]
    fn existing_shared_parent_mode_is_unchanged() {
        use std::os::unix::fs::PermissionsExt;

        let temp =
            std::env::temp_dir().join(format!("mesh-node-key-mode-{}", rand::random::<u64>()));
        std::fs::create_dir(&temp).unwrap();
        let shared = temp.join("shared");
        std::fs::create_dir(&shared).unwrap();
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();

        save_node_key_bytes_to_path(&shared.join("key"), &[7u8; NODE_KEY_BYTES]).unwrap();

        assert_eq!(
            std::fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(shared.join("key"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::remove_dir_all(temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn missing_parent_directories_are_created_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp =
            std::env::temp_dir().join(format!("mesh-node-key-missing-{}", rand::random::<u64>()));
        let private = temp.join("private");
        let nested = private.join("nested");
        let key_path = nested.join("key");

        save_node_key_bytes_to_path(&key_path, &[7u8; NODE_KEY_BYTES]).unwrap();

        for dir in [&private, &nested] {
            assert_eq!(
                std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(
            std::fs::metadata(key_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn relative_node_key_parent_is_created_in_the_current_directory() {
        // `mesh-llm auth` rotation passes explicitly relative key paths
        // through unchanged; the missing `keys/` ancestor must be created
        // under the working directory instead of failing with NotFound.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }
        let root = PathBuf::from(format!("mesh-node-key-relative-{}", rand::random::<u64>()));
        assert!(root.is_relative());
        let _cleanup = Cleanup(root.clone());
        let key_path = root.join("keys").join("node.key");
        let key = [9u8; NODE_KEY_BYTES];

        save_node_key_bytes_to_path(&key_path, &key).unwrap();

        assert!(key_path.is_file());
        assert_eq!(load_node_key_bytes_from_path(&key_path).unwrap(), key);
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    fn shared_decoy(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let decoy = dir.join("decoy");
        std::fs::write(&decoy, hex::encode([1u8; NODE_KEY_BYTES])).unwrap();
        std::fs::set_permissions(&decoy, std::fs::Permissions::from_mode(0o644)).unwrap();
        decoy
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_node_key_is_rejected_and_target_untouched() {
        let path = temp_node_key_path();
        let decoy = shared_decoy(path.parent().unwrap());
        std::os::unix::fs::symlink(&decoy, &path).unwrap();

        let error = load_node_key_bytes_from_path(&path).unwrap_err();

        assert!(matches!(error, CryptoError::InvalidKeyMaterial { .. }));
        assert_eq!(mode_of(&decoy), 0o644);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[cfg(unix)]
    #[test]
    fn path_swapped_after_open_does_not_redirect_chmod_or_read() {
        // #16: a local user who can write the key's directory swaps the key
        // for a symlink right after it is opened. The permission repair and
        // the read must still act on the opened key, not the link target.
        use std::os::unix::fs::PermissionsExt;

        let path = temp_node_key_path();
        let dir = path.parent().unwrap().to_path_buf();
        let key = [7u8; NODE_KEY_BYTES];
        std::fs::write(&path, hex::encode(key)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let decoy = shared_decoy(&dir);
        let moved = dir.join("moved");
        set_after_open_hook({
            let (path, decoy, moved) = (path.clone(), decoy.clone(), moved.clone());
            move || {
                std::fs::rename(&path, &moved).unwrap();
                std::os::unix::fs::symlink(&decoy, &path).unwrap();
            }
        });

        let loaded = load_node_key_bytes_from_path(&path).unwrap();

        assert_eq!(loaded, key);
        assert_eq!(mode_of(&decoy), 0o644);
        assert_eq!(mode_of(&moved), 0o600);
        std::fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn fifo_node_key_is_rejected_without_blocking() {
        let path = temp_node_key_path();
        let status = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());

        let error = load_node_key_bytes_from_path(&path).unwrap_err();

        assert!(matches!(error, CryptoError::InvalidKeyMaterial { .. }));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[cfg(unix)]
    #[test]
    fn loose_node_key_permissions_are_repaired() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_node_key_path();
        let key = [5u8; NODE_KEY_BYTES];
        std::fs::write(&path, hex::encode(key)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert_eq!(load_node_key_bytes_from_path(&path).unwrap(), key);
        assert_eq!(mode_of(&path), 0o600);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn rejects_wrong_length_node_key() {
        let path = temp_node_key_path();
        std::fs::write(&path, "abcd").unwrap();

        let error = load_node_key_bytes_from_path(&path).unwrap_err();

        assert!(matches!(error, CryptoError::InvalidKeyMaterial { .. }));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn node_key_path_honors_explicit_override() {
        // #1699: a second node process on one machine gets its own key via
        // MESH_LLM_NODE_KEY_PATH instead of silently sharing ~/.mesh-llm/key.
        let override_path = std::path::Path::new("/tmp/mesh-second-node.key");

        let resolved =
            resolve_node_key_path(Some(override_path.as_os_str().to_os_string())).unwrap();

        assert_eq!(resolved, override_path);
    }

    #[test]
    fn empty_node_key_override_falls_back_to_home() {
        // MESH_LLM_NODE_KEY_PATH= must not resolve to the working directory.
        let resolved = resolve_node_key_path(Some(std::ffi::OsString::new())).unwrap();

        assert_eq!(resolved, home_node_key_path().unwrap());
    }

    #[test]
    fn node_key_path_defaults_to_home_without_override() {
        // With no override set, the path must remain the historical
        // ~/.mesh-llm/key so existing installs keep their node identity.
        let resolved = resolve_node_key_path(None).unwrap();

        assert_eq!(resolved, home_node_key_path().unwrap());
    }
}
