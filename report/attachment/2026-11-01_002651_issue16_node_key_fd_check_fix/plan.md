# issue #16 修正: node 鍵の検査・chmod・読み取りを 1 つの file descriptor で行う

## Context

`mesh/crates/mesh-llm-identity/src/node_key.rs` は Unix で node 鍵を読むとき、`symlink_metadata(path)` で通常ファイルか確かめた後、`set_permissions(path)` と `read_to_string(path)` でパスを解決し直している（TOCTOU）。鍵の置き場所を `MESH_LLM_NODE_KEY_PATH` で他人が書き込める既存ディレクトリにした場合、検査直後に symlink へ差し替えられると、リンク先の chmod や別ファイルの読み取りが起きる。upstream にも jimoto-no-llm にも修正はまだ無い（2026-11-01 確認）。

目標: `O_NOFOLLOW` で開いた `File` に対して、種類の確認、`fchmod`、読み取りをすべて行う。通常の鍵の読み取りと 0600 への修復は今までどおり。

## 作業場所

```bash
git fetch origin
git worktree add -b fix/issue16-node-key-fd-check /home/ubuntu/projects/mesh-llm-worktrees/fix-issue16-node-key-fd-check origin/main
```

## 変更内容

### `mesh/crates/mesh-llm-identity/Cargo.toml`
- `[target.'cfg(unix)'.dependencies] libc = "0.2"` を追加（`O_NOFOLLOW` / `O_NONBLOCK` / `O_CLOEXEC` 定数のため。定数だけなので `#![forbid(unsafe_code)]` はそのまま。libc は既に lockfile にある）

### `mesh/crates/mesh-llm-identity/src/node_key.rs`（Unix 版）
- `ensure_private_node_key_file(path) -> Result<(), _>` を `open_private_node_key_file(path) -> Result<File, _>` に置き換える:
  1. `OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC).open(path)`。最後の要素が symlink なら `ELOOP` で失敗 → `InvalidKeyMaterial { "... is not a regular file" }` に変換（既存のエラー種別に揃える）。`O_NONBLOCK` は FIFO を置かれたときに open で固まらないため（通常ファイルの読み取りには影響しない）
  2. `file.metadata()`（fstat）で `is_file()` を確認。違えば同じエラー
  3. `mode & 0o077 != 0` なら `file.set_permissions(0o600)`（fchmod）
  4. `File` を返す
- `load_node_key_bytes_from_path`: 返った `File` から `read_to_string` する（パスを開き直さない）
- `save_node_key_bytes_to_path`: 書き込み前後の検査を新関数に差し替え（戻り値は捨てる）。挙動は同じ
- 非 Unix 版は現状維持（issue の対象は Unix）。ただし `load` から共通に呼べるよう、非 Unix 版も `File` を返す形にそろえる（`symlink_metadata` で確認後に `File::open`）

### 対象外（PR 本文に明記）
- 既存の親ディレクトリの権限チェック。既存テスト `existing_shared_parent_mode_is_unchanged` が 0755 の親を許す前提で、拒否すると挙動が変わるため。ディレクトリの中身を差し替えられても、今回の修正で symlink 先の chmod・読み取りは起きなくなる
- Windows の reparse point の扱い

## テスト（`node_key.rs` の tests に追加、`#[cfg(unix)]`）
- `symlinked_node_key_is_rejected_and_target_untouched`: 0644 の別ファイルへの symlink を鍵のパスに置く → load がエラー、リンク先の mode は 0644 のまま
- `opened_node_key_handle_survives_path_swap`: `open_private_node_key_file` で開いた後、パスを 0644 の別ファイルへの symlink に差し替え、その `File` で読む → 元の鍵の内容が読め、リンク先の mode は変わらない（差し替え後に操作しても影響を受けないことの決定的な再現）
- `fifo_node_key_is_rejected_without_blocking`: `mkfifo`（`std::process::Command`、unsafe 不可のため）で FIFO を置く → すぐエラー
- `loose_node_key_permissions_are_repaired`: 0644 の正しい鍵 → load 成功、mode 0600
- 修正を一時的に外して、追加テストが失敗することを確かめる

## 検証
（ワークステーションは sccache・just が無いので `RUSTC_WRAPPER=` を付け、レシピの中身を直接実行）
- `cargo fmt -p mesh-llm-identity` / `cargo fmt --all --check`
- `cargo check -p mesh-llm-identity --all-features`、`cargo clippy -p mesh-llm-identity --all-targets --all-features -- -D warnings`
- `cargo xtool repository affected-crates <変更パス>` で CI 対象を出し、その crate の `cargo test -p <crate>` を全部実行
- 出荷バイナリから到達するので `cargo check -p mesh-llm` と `cargo clippy -p mesh-llm --all-targets -- -D warnings`
- `cargo xtool repository conventional-commits --range origin/main..HEAD`

## コミット・PR
- `fix(identity): check, repair and read the node key through one descriptor`（Co-Authored-By トレーラーは付けない: メモリの規約）
- レポート `report/<JST タイムスタンプ>_issue16_node_key_fd_check_fix.md` を同じ worktree に書き、plan.md を添付。別コミット `docs(report): ...`
- push して PR（日本語本文、`Fixes #16`、問題/修正内容/今回は直していない点/テスト/レポートの構成で #30 にそろえる）
- 必要なら issue #16 に PR へのリンクは `Fixes` で足りるので追加コメントはしない
