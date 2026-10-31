# skill のインストール先が root の外に出る問題の修正 — 入口で名前を検査

- **実施日時**: 2026年10月31日 23:30 〜 23:42 JST (issue #14 の修正、テスト追加、PR 作成と CI 確認)
- **報告日時**: 2026年10月31日 23:42 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm には、plugin に同梱された「エージェント向けの手順書（skill）」を、Claude Code や Codex が読む場所へコピーする機能があります。`mesh-llm claude` などでエージェントを起動するときにも自動で動きます。issue #14 は、このコピー処理が skill の名前を確かめずに置き場所のパスにしているため、名前次第で決められた場所の外にファイルを作れてしまう、という問題でした。上書きを強制するオプションを付けると、外にある無関係なディレクトリを消すおそれもありました。

今回、コピー処理の入口で名前を確かめるようにしました。名前は「ディレクトリ名 1 つ分」でなければならず、絶対パスや親ディレクトリへの移動、区切り文字を含む名前は、ファイルに触る前にエラーにします。お試し実行（dry-run）でも同じエラーになります。

名前の形を決める既存の関数（英小文字・数字・ハイフンだけを許すもの）は、あえて使いませんでした。plugin の直下に置かれた skill は plugin 名をそのまま名前にするので、大文字や記号が入ることがあります。厳しい規則にすると、今まで問題なく入っていた skill が入らなくなるためです。

修正を外すと新しいテストが失敗し、修正を入れると通ることを確かめました。PR の CI もすべて通っています。

plugin の情報ファイルの名前がディレクトリ名と一致するかを確かめていない点と、plugin 名の検査が `.` や `..` を通してしまう点は、今回は直していません。どちらも不正な名前が入り込む経路になりえますが、今回の入口の検査で塞がります。このレポートは修正と同じ PR に含めています。PR マージ後に worktree を片付けます。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_234216_issue14_skill_name_path_check_fix/plan.md)

## 核心発見サマリ

**結論**: `install_skills` の先頭に `ensure_skill_name_is_single_component` を追加した。`Path::new(name).components()` がちょうど 1 個の `Component::Normal` で、それが元の文字列と等しいことを要求し、`""`・`.`・`..`・`../escape`・`a/b`・`demo/`・`demo/.`・`./demo`・絶対 path を拒否する（通常・dry-run とも、`force: true` でも何も作られない）。1 つでも不正な名前があればバッチ全体を拒否する。テストは `mesh-llm-skills` 8 passed、`mesh-llm-plugin-manager` 76 passed。修正を外すと拒否テスト 2 件が失敗する。PR #30 の CI（Clippy and tests / Commit convention / Rust format ほか）はすべて pass。

## 環境情報

- ワークステーション（aws-v100 は使っていない）
- worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue14-skill-name-path-check`（ブランチ `fix/issue14-skill-name-path-check`、base `65bcf2924`）
- コミット: `746edcdf2 fix(skills): reject skill names that are not a single path component`
- PR: https://github.com/miminashi/mesh-llm/pull/30
- 変更ファイル: `mesh/crates/mesh-llm-skills/src/lib.rs` のみ（+87 / −2）

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-issue14-skill-name-path-check
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -p mesh-llm-skills
cargo test -p mesh-llm-plugin-manager
cargo clippy -p mesh-llm-skills --all-targets -- -D warnings
cargo fmt --all --check
cargo run -q -p xtask -- repo-consistency no-console-print
cargo xtool repository conventional-commits --range origin/main..HEAD
cargo xtool repository affected-crates mesh/crates/mesh-llm-skills/src/lib.rs
```

## 結果詳細

| 確認 | 結果 |
|---|---|
| `cargo test -p mesh-llm-skills` | 8 passed（新規 3 件を含む） |
| 修正を一時的に外したテスト | `rejects_path_like_skill_names_before_touching_files` と `rejects_whole_batch_when_any_skill_name_is_invalid` が FAILED |
| `cargo test -p mesh-llm-plugin-manager` | 76 passed |
| clippy / fmt / no-console-print / conventional-commits | すべて pass |
| `affected-crates` の `test_crates` | `mesh-llm-skills` のみ |
| PR #30 の CI | 全ジョブ pass（fork の Clippy and tests で新テストが実行されたことをログで確認） |

追加したテスト:

- `accepts_single_component_skill_names`: `demo-skill`、`Cool.Plugin_1`、`.hidden` を受け入れる
- `rejects_path_like_skill_names_before_touching_files`: 上記の不正な名前を、通常・dry-run の両方で拒否し、home にも root の外にも何も作らない
- `rejects_whole_batch_when_any_skill_name_is_invalid`: 正しい名前と不正な名前が混ざっていると全体を拒否し、正しいほうも置かない

## 副次発見

- ワークステーションには `sccache` も `just` も入っていない。`.cargo/config.toml` の `rustc-wrapper = "sccache"` を `RUSTC_WRAPPER=` で外し、just のレシピ（`no-console-print`、`check-commits`）は中身の xtask コマンドを直接実行した
- `core.hooksPath` は設定されていない（`just hooks-install` 未実施）。設定はメインチェックアウトと worktree で共有されるので、今回は変えずに、コミット後に conventional-commits を手で確認した

## 残課題

- PR #30 のマージ後に `git worktree remove /home/ubuntu/projects/mesh-llm-worktrees/fix-issue14-skill-name-path-check`
- 範囲外とした 2 点（plugin metadata の `name` とディレクトリ名の照合、`is_valid_name` が `.` / `..` を通す点）を別 issue にするかどうか
