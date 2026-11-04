use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::source_ref::is_valid_name;

mod control_behavior;
mod exchange_access;
pub use exchange_access::InstalledOpenAiExchangeAccess;

pub use control_behavior::{
    InstalledPluginConditionOperator, InstalledPluginConditionValue,
    InstalledPluginConditionalDisable, InstalledPluginConflictRule,
    InstalledPluginControlAvailability, InstalledPluginControlAvailabilitySource,
    InstalledPluginControlBehavior, InstalledPluginControlCondition,
    InstalledPluginDisabledWritePolicy, InstalledPluginNumericControl,
    InstalledPluginOptionsSource, InstalledPluginTextFormat,
};

const METADATA_FILE: &str = "plugin-install.json";
/// Marks a default plugin the operator turned off while it was not installed.
const DEFAULT_OFF_FILE: &str = "default-off";
/// The directory under the store root that holds every plugin's files, one
/// `<root>/installed/<name>` each, beside the metadata directories
/// (`<root>/<name>`). A plugin named `installed` would keep its metadata in
/// it, and deleting that plugin would delete every plugin's files, so the
/// name is reserved ([`is_reserved_plugin_name`]).
pub const INSTALL_DIR_NAME: &str = "installed";
pub const SUPPORTED_PLUGIN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPluginManifestMetadata {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub openai_exchange_hook: Option<InstalledOpenAiExchangeAccess>,
    /// Provider connections the package asks for, by capability. Granted at
    /// install; an operator `operation_bindings` entry replaces them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operation_bindings: Vec<InstalledOperationBindingRequest>,
    /// The package asks to block/unblock routing to peers.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub peer_blocks: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub config_schema: Option<InstalledPluginConfigSchema>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub web_ui: Option<InstalledPluginWebUiMetadata>,
}

/// A provider connection requested by capability (see `OperationBindingManifest`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstalledOperationBindingRequest {
    pub name: String,
    pub capability: String,
    pub setup_operation: String,
    pub operations: Vec<String>,
    pub setup_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<InstalledPluginWebUiPageMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_sections: Vec<InstalledPluginWebUiConfigSectionMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributions: Vec<InstalledPluginWebUiContributionMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<InstalledPluginWebUiBundleMetadata>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub asset_root: Option<PathBuf>,
    #[serde(default)]
    pub validation: InstalledPluginWebUiValidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiPageMetadata {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub icon: Option<String>,
    pub route: String,
    pub bundle_id: String,
    pub entry_script: String,
    #[serde(default)]
    pub placement: InstalledPluginWebUiPagePlacement,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub host_header: Option<bool>,
}

/// A manifest-declared placement request, not a promotion decision: the
/// host also requires the persisted `web_ui_primary_tab` operator
/// preference before it renders a page as a primary tab. Absent on disk
/// (a `plugin-manifest.json` packaged before this field existed) defaults
/// to `Auxiliary` via `#[serde(default)]`, so older packages keep working.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginWebUiPagePlacement {
    #[default]
    Auxiliary,
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiConfigSectionMetadata {
    pub id: String,
    pub title: String,
    pub entry_script: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub parent_tab: Option<String>,
    pub bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiContributionMetadata {
    pub id: String,
    pub slot: String,
    pub label: String,
    pub bundle_id: String,
    pub entry_script: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiBundleMetadata {
    pub id: String,
    pub root_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginWebUiValidation {
    pub status: InstalledPluginWebUiValidationStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
}

impl Default for InstalledPluginWebUiValidation {
    fn default() -> Self {
        Self {
            status: InstalledPluginWebUiValidationStatus::Invalid,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginWebUiValidationStatus {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPluginConfigSchema {
    pub plugin_name: String,
    pub schema_version: u32,
    #[serde(default)]
    pub allow_unvalidated_config: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<InstalledPluginSettingSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPluginSettingSchema {
    pub key: String,
    pub value_schema: InstalledPluginValueSchema,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub default_json: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<InstalledPluginConstraint>,
    pub apply_mode: InstalledPluginApplyMode,
    pub restart_scope: InstalledPluginRestartScope,
    pub visibility: InstalledPluginVisibility,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub presentation: Option<InstalledPluginPresentationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub control_behavior: Option<InstalledPluginControlBehavior>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginPresentationMetadata {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category_order: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub setting_order: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub control_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub renderer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginValueSchema {
    pub kind: InstalledPluginValueKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub items: Option<Box<InstalledPluginValueSchema>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_properties: Vec<InstalledPluginObjectProperty>,
    #[serde(default)]
    pub allow_additional_properties: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPluginObjectProperty {
    pub key: String,
    pub value_schema: InstalledPluginValueSchema,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginValueKind {
    Boolean,
    Integer,
    Float,
    String,
    Path,
    Url,
    Enum,
    Array,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginApplyMode {
    StaticOnLoad,
    DynamicValidationOnly,
    DynamicApply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginRestartScope {
    None,
    ModelReload,
    ProcessRestart,
    MeshRestart,
    PluginProcess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPluginVisibility {
    User,
    Advanced,
    Hidden,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstalledPluginConstraint {
    NonEmpty,
    Positive,
    Range {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        min: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        max: Option<String>,
    },
    AllowedValues {
        values: Vec<String>,
    },
    Requires {
        key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPluginMetadata {
    pub name: String,
    pub source_repository: String,
    pub installed_version: String,
    pub target_triple: String,
    pub downloaded_asset_name: String,
    pub install_path: PathBuf,
    pub enabled: bool,
    #[serde(default)]
    pub default_managed: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manifest: Option<InstalledPluginManifestMetadata>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_protocol_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_error: Option<String>,
    /// The SHA-256 the installed archive was verified against when it was
    /// installed: the catalog pin for a catalog install, the digest GitHub
    /// reported for a GitHub reference. Absent for a local `--archive` install
    /// and for metadata written before installs recorded provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_sha256: Option<String>,
}

impl InstalledPluginMetadata {
    /// The plugin's executable, named for the target it was installed for,
    /// the same name the install checked. A stored triple this build does not
    /// support (a hand-written or test record) falls back to the host's suffix.
    pub fn executable_path(&self) -> PathBuf {
        let suffix = crate::target::PluginTarget::executable_suffix_for(&self.target_triple)
            .unwrap_or(std::env::consts::EXE_SUFFIX);
        self.install_path.join(format!("{}{suffix}", self.name))
    }

    pub fn web_ui_asset_root_path(&self) -> Option<PathBuf> {
        let web_ui = self.manifest.as_ref()?.web_ui.as_ref()?;
        if web_ui.validation.status != InstalledPluginWebUiValidationStatus::Valid {
            return None;
        }
        web_ui
            .asset_root
            .as_ref()
            .map(|asset_root| self.install_path.join(asset_root))
    }
}

#[derive(Debug, Clone)]
pub struct PluginStore {
    root: PathBuf,
}

/// A store directory whose `plugin-install.json` names a different plugin
/// than the directory it sits in, or places the plugin's files somewhere
/// other than `<root>/installed/<name>`. Its metadata is not used: every
/// other part of the host trusts `name` and `install_path` (`plugins delete`
/// used to remove `install_path`), so both must be what the store derives
/// from the checked directory name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPluginMetadata {
    /// The store directory name, which passed the plugin name check.
    pub directory: String,
    pub reason: String,
}

/// What [`PluginStore::list_checked`] found in the store.
#[derive(Debug, Clone, Default)]
pub struct PluginListing {
    pub plugins: Vec<InstalledPluginMetadata>,
    pub rejected: Vec<RejectedPluginMetadata>,
}

impl PluginStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory holding every installed plugin's files.
    pub fn install_root(&self) -> PathBuf {
        self.root.join(INSTALL_DIR_NAME)
    }

    /// Where the files of plugin `name` are installed: the only
    /// `install_path` the store accepts in that plugin's metadata.
    pub fn install_path(&self, name: &str) -> PathBuf {
        self.install_root().join(name)
    }

    pub fn save(&self, metadata: &InstalledPluginMetadata) -> Result<()> {
        validate_plugin_name(&metadata.name)?;
        // Never write a record that loading would reject.
        if let Some(reason) = self.install_path_mismatch(&metadata.name, metadata) {
            bail!(reason);
        }
        let plugin_dir = self.plugin_dir(&metadata.name);
        fs::create_dir_all(&plugin_dir).with_context(|| {
            format!("create plugin metadata directory {}", plugin_dir.display())
        })?;
        let metadata_path = self.metadata_path(&metadata.name);
        let temp_path = metadata_path.with_extension("json.tmp");
        let contents = serde_json::to_vec_pretty(metadata)?;
        fs::write(&temp_path, contents)
            .with_context(|| format!("write plugin metadata {}", temp_path.display()))?;
        fs::rename(&temp_path, &metadata_path).with_context(|| {
            format!(
                "replace plugin metadata {} with {}",
                metadata_path.display(),
                temp_path.display()
            )
        })?;
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<InstalledPluginMetadata> {
        self.try_load(name)?
            .with_context(|| format!("plugin '{name}' is not installed"))
    }

    pub fn try_load(&self, name: &str) -> Result<Option<InstalledPluginMetadata>> {
        let Some(metadata) = self.read_metadata(name)? else {
            return Ok(None);
        };
        if let Some(reason) = self.metadata_mismatch(name, &metadata) {
            bail!(reason);
        }
        Ok(Some(metadata))
    }

    /// Read and parse `<name>/plugin-install.json` without checking that the
    /// `name` and `install_path` inside it match. Callers must check them
    /// with `metadata_mismatch`.
    fn read_metadata(&self, name: &str) -> Result<Option<InstalledPluginMetadata>> {
        validate_plugin_name(name)?;
        let metadata_path = self.metadata_path(name);
        if !metadata_path.exists() {
            return Ok(None);
        }
        let contents = fs::read(&metadata_path)
            .with_context(|| format!("read plugin metadata {}", metadata_path.display()))?;
        Ok(Some(serde_json::from_slice(&contents).with_context(
            || format!("parse plugin metadata {}", metadata_path.display()),
        )?))
    }

    /// Why the metadata stored under `name` cannot be used, if it cannot.
    fn metadata_mismatch(&self, name: &str, metadata: &InstalledPluginMetadata) -> Option<String> {
        self.name_mismatch(name, metadata)
            .or_else(|| self.install_path_mismatch(name, metadata))
    }

    /// `save` places metadata in the directory named by `metadata.name`, so
    /// the two always match unless the file was edited by hand.
    fn name_mismatch(&self, name: &str, metadata: &InstalledPluginMetadata) -> Option<String> {
        (metadata.name != name).then(|| {
            format!(
                "plugin metadata {} names plugin {:?}, but it is stored under {:?}",
                self.metadata_path(name).display(),
                metadata.name,
                name
            )
        })
    }

    /// Every install extracts a plugin to exactly `install_path(name)`, so a
    /// different `install_path` was edited by hand (or written for another
    /// store root). It must be that exact directory, not merely one under the
    /// install root: the install root itself, or another plugin's directory,
    /// would let `delete` or a launch reach files that are not this plugin's.
    /// The paths are compared as written, component by component, so a `..`
    /// or a different spelling of the same directory is refused too.
    fn install_path_mismatch(
        &self,
        name: &str,
        metadata: &InstalledPluginMetadata,
    ) -> Option<String> {
        let expected = self.install_path(name);
        (metadata.install_path != expected).then(|| {
            format!(
                "plugin metadata {} places plugin {:?} at {}, but its files must be at {}",
                self.metadata_path(name).display(),
                name,
                metadata.install_path.display(),
                expected.display()
            )
        })
    }

    pub fn load_optional(&self, name: &str) -> Result<Option<InstalledPluginMetadata>> {
        self.try_load(name)
    }

    /// The installed plugins. A directory whose metadata names a different
    /// plugin, or places its files elsewhere, is left out, so one edited file
    /// cannot stop the others from loading; use [`Self::list_checked`] to
    /// report it.
    pub fn list(&self) -> Result<Vec<InstalledPluginMetadata>> {
        Ok(self.list_checked()?.plugins)
    }

    pub fn list_checked(&self) -> Result<PluginListing> {
        let mut listing = PluginListing::default();
        if !self.root.exists() {
            return Ok(listing);
        }

        for entry in fs::read_dir(&self.root)
            .with_context(|| format!("read plugin store {}", self.root.display()))?
        {
            let entry = entry
                .with_context(|| format!("read plugin store entry {}", self.root.display()))?;
            if !entry
                .file_type()
                .with_context(|| format!("read file type for {}", entry.path().display()))?
                .is_dir()
            {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !is_valid_name(&name) {
                continue;
            }
            let Some(metadata) = self.read_metadata(&name)? else {
                continue;
            };
            match self.metadata_mismatch(&name, &metadata) {
                None => listing.plugins.push(metadata),
                Some(reason) => listing.rejected.push(RejectedPluginMetadata {
                    directory: name,
                    reason,
                }),
            }
        }
        listing
            .plugins
            .sort_by(|left, right| left.name.cmp(&right.name));
        listing
            .rejected
            .sort_by(|left, right| left.directory.cmp(&right.directory));
        Ok(listing)
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<InstalledPluginMetadata> {
        let mut metadata = self.load(name)?;
        metadata.enabled = enabled;
        self.save(&metadata)?;
        Ok(metadata)
    }

    /// Remove the plugin's files and its metadata directory. The files are
    /// found by the name, never by the metadata's `install_path`, so an
    /// edited record cannot point `delete` at another directory (#54).
    pub fn delete(&self, name: &str) -> Result<()> {
        validate_plugin_name(name)?;
        let install_path = self.install_path(name);
        // `symlink_metadata`, not `exists`: a link there is removed itself,
        // and is not followed into what it points at.
        if let Ok(file_type) = fs::symlink_metadata(&install_path).map(|meta| meta.file_type()) {
            let removed = if file_type.is_dir() {
                fs::remove_dir_all(&install_path)
            } else {
                // A Windows directory link is removed with `remove_dir`.
                fs::remove_file(&install_path).or_else(|_| fs::remove_dir(&install_path))
            };
            removed.with_context(|| format!("delete plugin install {}", install_path.display()))?;
        }
        let plugin_dir = self.plugin_dir(name);
        if is_reserved_plugin_name(name) {
            // Installed before the name was reserved: its metadata directory
            // is the one holding every plugin's files, so remove only the
            // files the store wrote there.
            for file in [METADATA_FILE, DEFAULT_OFF_FILE] {
                let path = plugin_dir.join(file);
                if path.exists() {
                    fs::remove_file(&path)
                        .with_context(|| format!("delete plugin metadata {}", path.display()))?;
                }
            }
        } else if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)
                .with_context(|| format!("delete plugin metadata {}", plugin_dir.display()))?;
        }
        Ok(())
    }

    /// Record whether a default plugin is turned off: while it is, the
    /// installers and `mesh-llm update` do not install it. This works when
    /// the plugin is not installed: `plugins delete` leaves this record for a
    /// default, and `plugins disable` leaves it for a default not installed.
    pub fn set_default_turned_off(&self, name: &str, off: bool) -> Result<()> {
        validate_plugin_name(name)?;
        let marker = self.plugin_dir(name).join(DEFAULT_OFF_FILE);
        if off {
            let plugin_dir = self.plugin_dir(name);
            fs::create_dir_all(&plugin_dir).with_context(|| {
                format!("create plugin metadata directory {}", plugin_dir.display())
            })?;
            fs::write(&marker, b"").with_context(|| format!("write {}", marker.display()))?;
        } else if marker.exists() {
            fs::remove_file(&marker).with_context(|| format!("remove {}", marker.display()))?;
        }
        Ok(())
    }

    /// Whether `plugins disable` turned this default off while it was not installed.
    pub fn default_turned_off(&self, name: &str) -> bool {
        is_valid_name(name) && self.plugin_dir(name).join(DEFAULT_OFF_FILE).exists()
    }

    fn plugin_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn metadata_path(&self, name: &str) -> PathBuf {
        self.plugin_dir(name).join(METADATA_FILE)
    }
}

pub fn default_store_root() -> Result<PathBuf> {
    // A non-UTF-8 value stays unset: the path is saved in JSON metadata.
    store_root_from(
        std::env::var("MESH_LLM_PLUGIN_DIR").ok().map(Into::into),
        dirs::home_dir(),
    )
}

/// An empty `MESH_LLM_PLUGIN_DIR` (what a systemd `Environment=` line that
/// clears it sets) counts as unset, not as the working directory.
fn store_root_from(
    plugin_dir: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = plugin_dir.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = home.context("Cannot determine home directory")?;
    Ok(home.join(".mesh-llm").join("plugins"))
}

/// Whether `name` cannot name a plugin to install: its metadata directory
/// would be [`INSTALL_DIR_NAME`]. Case is ignored, because a case-insensitive
/// file system (the macOS and Windows defaults) resolves `Installed` there too.
pub fn is_reserved_plugin_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(INSTALL_DIR_NAME)
}

/// Refuse a name no plugin may be installed under. Every install path checks
/// this after it has settled the plugin's name, before it writes anything.
pub(crate) fn ensure_installable_plugin_name(name: &str) -> Result<()> {
    if is_reserved_plugin_name(name) {
        bail!(
            "plugin name '{name}' is reserved: the plugin store keeps every plugin's files \
             in its '{INSTALL_DIR_NAME}' directory"
        );
    }
    Ok(())
}

fn validate_plugin_name(name: &str) -> Result<()> {
    if is_valid_name(name) {
        Ok(())
    } else {
        bail!("invalid plugin name: {name}")
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn an_empty_plugin_dir_falls_back_to_home() {
        let home = PathBuf::from("/home/node");
        assert_eq!(
            store_root_from(Some("".into()), Some(home.clone())).unwrap(),
            home.join(".mesh-llm").join("plugins")
        );
        assert_eq!(
            store_root_from(Some("/srv/plugins".into()), Some(home)).unwrap(),
            PathBuf::from("/srv/plugins")
        );
    }

    #[test]
    fn executable_path_follows_the_stored_target_on_any_host() {
        let mut item = metadata("demo");
        for (triple, executable) in [
            ("x86_64-pc-windows-msvc", "demo.exe".to_string()),
            ("aarch64-pc-windows-msvc", "demo.exe".to_string()),
            ("x86_64-unknown-linux-gnu", "demo".to_string()),
            ("aarch64-apple-darwin", "demo".to_string()),
            (
                "test-target",
                format!("demo{}", std::env::consts::EXE_SUFFIX),
            ),
        ] {
            item.target_triple = triple.to_string();
            assert_eq!(
                item.executable_path(),
                item.install_path.join(&executable),
                "{triple}"
            );
        }
    }

    fn metadata(name: &str) -> InstalledPluginMetadata {
        InstalledPluginMetadata {
            name: name.to_string(),
            source_repository: "https://github.com/mesh-llm/blackboard".to_string(),
            installed_version: "v1.0.0".to_string(),
            target_triple: "aarch64-apple-darwin".to_string(),
            downloaded_asset_name: "blackboard-v1.0.0-aarch64-apple-darwin.tar.gz".to_string(),
            install_path: PathBuf::from("/tmp/plugins/blackboard"),
            enabled: true,
            default_managed: false,
            manifest: Some(InstalledPluginManifestMetadata {
                openai_exchange_hook: None,
                operation_bindings: Vec::new(),
                peer_blocks: false,
                config_schema: Some(InstalledPluginConfigSchema {
                    plugin_name: name.to_string(),
                    schema_version: SUPPORTED_PLUGIN_SCHEMA_VERSION,
                    allow_unvalidated_config: false,
                    settings: vec![InstalledPluginSettingSchema {
                        key: "retention_days".to_string(),
                        value_schema: InstalledPluginValueSchema {
                            kind: InstalledPluginValueKind::Integer,
                            enum_values: Vec::new(),
                            items: None,
                            object_properties: Vec::new(),
                            allow_additional_properties: false,
                        },
                        required: true,
                        default_json: Some("14".to_string()),
                        constraints: vec![InstalledPluginConstraint::Range {
                            min: Some("1".to_string()),
                            max: Some("365".to_string()),
                        }],
                        apply_mode: InstalledPluginApplyMode::DynamicValidationOnly,
                        restart_scope: InstalledPluginRestartScope::PluginProcess,
                        visibility: InstalledPluginVisibility::User,
                        description: Some("How long to retain entries.".to_string()),
                        presentation: Some(InstalledPluginPresentationMetadata {
                            label: Some("Retention days".to_string()),
                            help: Some("How long to retain entries.".to_string()),
                            category_id: Some("retention".to_string()),
                            category_label: Some("Retention".to_string()),
                            category_summary: Some("Retention settings".to_string()),
                            category_order: Some(10),
                            setting_order: Some(20),
                            unit: Some("days".to_string()),
                            placeholder: None,
                            control_hint: Some("number".to_string()),
                            renderer_id: None,
                        }),
                        control_behavior: None,
                    }],
                }),
                web_ui: None,
            }),
            last_protocol_version: Some(2),
            last_status: Some("running".to_string()),
            last_error: None,
            verified_sha256: None,
        }
    }

    /// `metadata(name)` with the `install_path` the store derives for it.
    fn stored(store: &PluginStore, name: &str) -> InstalledPluginMetadata {
        let mut item = metadata(name);
        item.install_path = store.install_path(name);
        item
    }

    #[test]
    fn saves_loads_and_lists_metadata() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());

        store.save(&stored(&store, "blackboard")).unwrap();
        store.save(&stored(&store, "notes")).unwrap();

        let loaded = store.load("blackboard").unwrap();
        assert_eq!(loaded.name, "blackboard");
        assert!(loaded.enabled);
        assert_eq!(
            loaded
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.config_schema.as_ref())
                .map(|schema| schema.schema_version),
            Some(SUPPORTED_PLUGIN_SCHEMA_VERSION)
        );
        assert_eq!(
            loaded
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.config_schema.as_ref())
                .and_then(|schema| schema.settings.first())
                .and_then(|setting| setting.presentation.as_ref())
                .and_then(|presentation| presentation.unit.as_deref()),
            Some("days")
        );
        assert_eq!(loaded.last_protocol_version, Some(2));

        let listed = store.list().unwrap();
        assert_eq!(
            listed
                .iter()
                .map(|plugin| plugin.name.as_str())
                .collect::<Vec<_>>(),
            vec!["blackboard", "notes"]
        );
    }

    #[test]
    fn updates_enabled_state() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        store.save(&stored(&store, "blackboard")).unwrap();

        let disabled = store.set_enabled("blackboard", false).unwrap();
        assert!(!disabled.enabled);
        assert!(!store.load("blackboard").unwrap().enabled);
    }

    #[test]
    fn load_optional_distinguishes_missing_metadata() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());

        assert!(store.load_optional("blackboard").unwrap().is_none());

        store.save(&stored(&store, "blackboard")).unwrap();
        assert_eq!(
            store.load_optional("blackboard").unwrap().unwrap().name,
            "blackboard"
        );
    }

    #[test]
    fn deletes_metadata_directory() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        let install_path = temp.path().join(INSTALL_DIR_NAME).join("blackboard");
        std::fs::create_dir_all(&install_path).unwrap();
        std::fs::write(install_path.join("blackboard"), b"executable").unwrap();
        store.save(&stored(&store, "blackboard")).unwrap();
        let notes_files = store.install_path("notes");
        std::fs::create_dir_all(&notes_files).unwrap();
        store.save(&stored(&store, "notes")).unwrap();

        store.delete("blackboard").unwrap();
        assert_eq!(
            store
                .list()
                .unwrap()
                .into_iter()
                .map(|item| item.name)
                .collect::<Vec<_>>(),
            ["notes"]
        );
        assert!(!install_path.exists());
        assert!(!temp.path().join("blackboard").exists());
        assert!(notes_files.is_dir());
    }

    #[test]
    fn delete_removes_the_files_of_a_plugin_without_metadata() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        let install_path = store.install_path("blackboard");
        std::fs::create_dir_all(&install_path).unwrap();

        store.delete("blackboard").unwrap();
        assert!(!install_path.exists());
        assert!(store.install_root().is_dir());
    }

    /// Hand-edit `<dir>/plugin-install.json` so its `install_path` is `path`.
    fn relocate_in_metadata(store_root: &Path, dir: &str, path: &Path) {
        let file = store_root.join(dir).join(METADATA_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        value["install_path"] = serde_json::to_value(path).unwrap();
        std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    }

    /// The paths an edited record could use to reach files that are not the
    /// plugin's: outside the store, the store or install root itself, another
    /// plugin's files, below its own, or its own spelled with `..` or `.`.
    fn foreign_install_paths(store: &PluginStore, outside: &Path) -> Vec<PathBuf> {
        let root = store.root().to_path_buf();
        let install_root = store.install_root();
        vec![
            outside.to_path_buf(),
            root.clone(),
            install_root.clone(),
            install_root.join("notes"),
            install_root.join("demo").join("nested"),
            install_root.join("other").join("..").join("demo"),
            root.join("..")
                .join(root.file_name().unwrap())
                .join(INSTALL_DIR_NAME)
                .join("demo"),
            PathBuf::from("installed").join("demo"),
        ]
    }

    #[test]
    fn load_and_save_reject_an_install_path_other_than_the_plugins_own() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        store.save(&stored(&store, "demo")).unwrap();

        for path in foreign_install_paths(&store, outside.path()) {
            relocate_in_metadata(temp.path(), "demo", &path);
            let error = store.load("demo").unwrap_err().to_string();
            assert!(error.contains("must be at"), "{}: {error}", path.display());
            assert!(store.try_load("demo").is_err(), "{}", path.display());
            assert!(
                store.set_enabled("demo", false).is_err(),
                "{}",
                path.display()
            );

            let mut item = stored(&store, "demo");
            item.install_path = path.clone();
            assert!(store.save(&item).is_err(), "save {}", path.display());
        }
    }

    #[test]
    fn list_skips_and_reports_metadata_placing_files_elsewhere() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        store.save(&stored(&store, "blackboard")).unwrap();
        store.save(&stored(&store, "demo")).unwrap();
        store.save(&stored(&store, "notes")).unwrap();
        relocate_in_metadata(temp.path(), "demo", outside.path());
        // Another plugin's files, inside the install root, are refused too.
        relocate_in_metadata(temp.path(), "notes", &store.install_path("blackboard"));

        let listing = store.list_checked().unwrap();
        assert_eq!(
            listing
                .plugins
                .iter()
                .map(|plugin| plugin.name.as_str())
                .collect::<Vec<_>>(),
            ["blackboard"]
        );
        assert_eq!(
            listing
                .rejected
                .iter()
                .map(|rejected| rejected.directory.as_str())
                .collect::<Vec<_>>(),
            ["demo", "notes"]
        );
        assert!(
            listing.rejected[0]
                .reason
                .contains(&outside.path().display().to_string()),
            "{}",
            listing.rejected[0].reason
        );
        assert_eq!(store.list().unwrap().len(), 1);
    }

    /// #54: `delete` used to remove whatever directory the record's
    /// `install_path` named. It now removes only the plugin's own files.
    #[test]
    fn delete_never_removes_the_directory_an_edited_record_names() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("keep"), b"keep").unwrap();
        let store = PluginStore::new(temp.path());

        for path in foreign_install_paths(&store, outside.path()) {
            for name in ["blackboard", "demo", "notes"] {
                let files = store.install_path(name);
                std::fs::create_dir_all(&files).unwrap();
                std::fs::write(files.join(name), b"executable").unwrap();
                store.save(&stored(&store, name)).unwrap();
            }
            relocate_in_metadata(temp.path(), "demo", &path);

            store.delete("demo").unwrap();

            assert_eq!(
                std::fs::read(outside.path().join("keep")).unwrap(),
                b"keep",
                "{}",
                path.display()
            );
            assert!(!store.install_path("demo").exists(), "{}", path.display());
            assert!(!temp.path().join("demo").exists(), "{}", path.display());
            for name in ["blackboard", "notes"] {
                assert!(
                    store.install_path(name).join(name).is_file(),
                    "{name} after {}",
                    path.display()
                );
                assert_eq!(store.load(name).unwrap().name, name);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn delete_removes_a_link_in_place_of_the_files_not_its_target() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("keep"), b"keep").unwrap();
        let store = PluginStore::new(temp.path());
        std::fs::create_dir_all(store.install_root()).unwrap();
        std::os::unix::fs::symlink(outside.path(), store.install_path("demo")).unwrap();
        store.save(&stored(&store, "demo")).unwrap();

        store.delete("demo").unwrap();

        assert!(std::fs::symlink_metadata(store.install_path("demo")).is_err());
        assert_eq!(std::fs::read(outside.path().join("keep")).unwrap(), b"keep");
    }

    /// A plugin named `installed`, installed before the name was reserved,
    /// keeps its metadata in the directory holding every plugin's files.
    /// Deleting it removes its own files and leaves the other plugins'.
    #[test]
    fn deleting_a_plugin_named_installed_keeps_the_other_plugins_files() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        let install_root = temp.path().join(INSTALL_DIR_NAME);
        let demo_files = install_root.join("demo");
        std::fs::create_dir_all(&demo_files).unwrap();
        std::fs::write(demo_files.join("demo"), b"executable").unwrap();
        let mut demo = metadata("demo");
        demo.install_path = demo_files.clone();
        store.save(&demo).unwrap();
        let installed_files = install_root.join("installed");
        std::fs::create_dir_all(&installed_files).unwrap();
        let mut installed = metadata("installed");
        installed.install_path = installed_files.clone();
        store.save(&installed).unwrap();
        store.set_default_turned_off("installed", true).unwrap();
        assert_eq!(store.plugin_dir("installed"), install_root);

        store.delete("installed").unwrap();

        assert!(demo_files.join("demo").is_file());
        assert!(!installed_files.exists());
        assert!(!install_root.join(METADATA_FILE).exists());
        assert!(!install_root.join(DEFAULT_OFF_FILE).exists());
        let names: Vec<_> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|item| item.name)
            .collect();
        assert_eq!(names, ["demo"]);
    }

    #[test]
    fn installed_is_reserved_in_any_case() {
        for name in ["installed", "Installed", "INSTALLED"] {
            assert!(is_reserved_plugin_name(name), "{name}");
            let error = ensure_installable_plugin_name(name)
                .unwrap_err()
                .to_string();
            assert!(error.contains("reserved"), "{error}");
        }
        for name in ["demo", "installed-plugin", "install", "installed2"] {
            assert!(!is_reserved_plugin_name(name), "{name}");
            ensure_installable_plugin_name(name).unwrap();
        }
    }

    /// Hand-edit `<dir>/plugin-install.json` so its `name` differs from `dir`.
    fn rename_in_metadata(store_root: &Path, dir: &str, name: &str) {
        let path = store_root.join(dir).join(METADATA_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["name"] = serde_json::Value::String(name.to_string());
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    }

    #[test]
    fn load_rejects_metadata_naming_another_plugin() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        store.save(&stored(&store, "demo")).unwrap();
        rename_in_metadata(temp.path(), "demo", "../../escape");

        let error = store.load("demo").unwrap_err().to_string();
        assert!(error.contains("\"../../escape\""), "{error}");
        assert!(error.contains("\"demo\""), "{error}");
        assert!(store.try_load("demo").is_err());
        assert!(store.set_enabled("demo", false).is_err());
    }

    #[test]
    fn list_skips_and_reports_metadata_naming_another_plugin() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        store.save(&stored(&store, "blackboard")).unwrap();
        store.save(&stored(&store, "demo")).unwrap();
        store.save(&stored(&store, "notes")).unwrap();
        rename_in_metadata(temp.path(), "demo", "../../escape");
        // A valid name that belongs to another plugin is rejected too.
        rename_in_metadata(temp.path(), "notes", "blackboard");

        let names = |plugins: &[InstalledPluginMetadata]| {
            plugins
                .iter()
                .map(|plugin| plugin.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&store.list().unwrap()), vec!["blackboard"]);

        let listing = store.list_checked().unwrap();
        assert_eq!(names(&listing.plugins), vec!["blackboard"]);
        assert_eq!(
            listing
                .rejected
                .iter()
                .map(|rejected| rejected.directory.as_str())
                .collect::<Vec<_>>(),
            vec!["demo", "notes"]
        );
        assert!(listing.rejected[0].reason.contains("\"../../escape\""));
    }

    #[test]
    fn delete_of_mismatched_metadata_keeps_its_install_path() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        let elsewhere = TempDir::new().unwrap();
        store.save(&stored(&store, "demo")).unwrap();
        relocate_in_metadata(temp.path(), "demo", elsewhere.path());
        rename_in_metadata(temp.path(), "demo", "other");

        store.delete("demo").unwrap();
        assert!(!temp.path().join("demo").exists());
        assert!(
            elsewhere.path().exists(),
            "metadata that fails the name check must not pick what delete removes"
        );
    }

    /// `.` and `..` would name the store root and its parent (by default
    /// `~/.mesh-llm`, which holds the node key): no store operation may take
    /// them as a plugin name.
    #[test]
    fn dot_names_never_reach_the_store_root_or_its_parent() {
        let temp = TempDir::new().unwrap();
        let parent = temp.path().join("dot-mesh-llm");
        let root = parent.join("plugins");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(parent.join("key"), b"node key").unwrap();
        std::fs::write(parent.join(METADATA_FILE), b"{}").unwrap();
        std::fs::write(root.join(METADATA_FILE), b"{}").unwrap();
        let store = PluginStore::new(&root);
        store.save(&stored(&store, "blackboard")).unwrap();

        for name in [".", ".."] {
            assert!(store.delete(name).is_err(), "delete {name}");
            assert!(store.try_load(name).is_err(), "try_load {name}");
            assert!(store.load(name).is_err(), "load {name}");
            assert!(store.save(&metadata(name)).is_err(), "save {name}");
            assert!(
                store.set_default_turned_off(name, true).is_err(),
                "set_default_turned_off {name}"
            );
            assert!(
                store.set_default_turned_off(name, false).is_err(),
                "set_default_turned_off {name}"
            );
            assert!(!store.default_turned_off(name), "default_turned_off {name}");
        }

        assert_eq!(std::fs::read(parent.join("key")).unwrap(), b"node key");
        assert_eq!(std::fs::read(parent.join(METADATA_FILE)).unwrap(), b"{}");
        assert_eq!(std::fs::read(root.join(METADATA_FILE)).unwrap(), b"{}");
        assert!(!parent.join(DEFAULT_OFF_FILE).exists());
        assert!(!root.join(DEFAULT_OFF_FILE).exists());
        assert_eq!(store.load("blackboard").unwrap().name, "blackboard");
    }

    #[test]
    fn list_ignores_non_metadata_directories() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        std::fs::create_dir_all(temp.path().join("installed").join("blackboard")).unwrap();
        store.save(&stored(&store, "blackboard")).unwrap();

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "blackboard");
    }

    #[test]
    fn load_legacy_metadata_without_control_behavior() {
        let temp = TempDir::new().unwrap();
        let store = PluginStore::new(temp.path());
        let plugin_dir = temp.path().join("blackboard");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join(METADATA_FILE),
            r#"{
  "name": "blackboard",
  "source_repository": "https://github.com/mesh-llm/blackboard",
  "installed_version": "v1.0.0",
  "target_triple": "aarch64-apple-darwin",
  "downloaded_asset_name": "blackboard-v1.0.0-aarch64-apple-darwin.tar.gz",
  "install_path": "/tmp/plugins/blackboard",
  "enabled": true,
  "manifest": {
    "config_schema": {
      "plugin_name": "blackboard",
      "schema_version": 1,
      "allow_unvalidated_config": false,
      "settings": [
        {
          "key": "retention_days",
          "value_schema": { "kind": "integer" },
          "required": true,
          "apply_mode": "dynamic_validation_only",
          "restart_scope": "plugin_process",
          "visibility": "user"
        }
      ]
    }
  }
}"#,
        )
        .unwrap();

        relocate_in_metadata(temp.path(), "blackboard", &store.install_path("blackboard"));

        let loaded = store.load("blackboard").unwrap();
        let setting = loaded
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.config_schema.as_ref())
            .and_then(|schema| schema.settings.first())
            .expect("legacy setting should load");

        assert!(setting.control_behavior.is_none());
    }
}
