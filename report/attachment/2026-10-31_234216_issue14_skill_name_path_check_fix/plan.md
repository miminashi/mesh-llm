# issue #14 修正: install_skills の入口で skill 名を path として検査する

## Context

公開 API `mesh_llm_skills::install_skills` は `SkillPackage.name` を検査せずに `target.root.join(&skill.name)` を置き場所にしている（`mesh/crates/mesh-llm-skills/src/lib.rs:200`）。絶対 path なら root が捨てられ、`..` なら root の外に出るので、Skill root の外に SKILL.md を作れる。`--force` のときは `replace_skill_dir` が置き場所を `remove_dir_all` するので、外のディレクトリを消すおそれもある。今の呼び出し元は plugin manager だけで、plugin 名は `is_valid_name` を通るが、`.` / `..` は文字種として通ってしまい、metadata JSON の `name` はディレクトリ名と照合されていない。安全性を呼び出し側に任せず、API の入口で拒否する。

## 方針

`mesh/crates/mesh-llm-skills/src/lib.rs` だけを変える。

1. `install_skills` の先頭（`resolve_targets` やファイル操作の前）で、全 skill の名前を検査する。1 つでも不正なら `bail!` して何もしない（all-or-nothing）。dry-run も同じ経路なので同じエラーになる
2. 検査関数 `ensure_skill_name_is_single_component(name: &str) -> Result<()>`（private）:
   - `Path::new(name).components()` がちょうど 1 個で、それが `Component::Normal(c)` かつ `c == name`（`foo/` や `foo/.` のような正規化で形が変わるものも拒否）
   - これで空文字・絶対 path（`RootDir` / Windows の `Prefix`）・`.`・`..`・`a/b` をまとめて拒否する
   - エラー文: `invalid skill name {name:?}: must be a single path component`
3. `is_valid_skill_name`（英小文字・数字・`-`）は使わない。plugin 直下の SKILL.md は plugin 名（大文字・`.`・`_` を含みうる）を skill 名にするので、使うと既存の安全な名前が壊れる。issue の「既存の安全な名前と marker・force・conflict の動作は変えない」に合わせる
4. temp dir 名（`.mesh-llm-skill-{pid}-{name}`）も同じ名前から作るので、この検査で一緒に守られる

範囲外（PR 本文に記載だけする）: plugin store が metadata の `name` とディレクトリ名を照合しない点、`is_valid_name` が `.` / `..` を通す点。入口の検査でどちらの経路も塞がるので、今回は直さない。

## テスト（同ファイルの `mod tests` に追加）

- `rejects_path_like_skill_names_before_touching_files`: 名前 `""`, `"."`, `".."`, `"../escape"`, `"a/b"`, `"demo/"`, 絶対 path（`temp/outside`）それぞれについて、dry_run false / true の両方で `install_skills` が Err を返し、`home_dir` も `outside` も作られていないこと（`detected_only: false`, `force: true` で最も危ない条件にする）
- `rejects_whole_batch_when_any_skill_name_is_invalid`: 正しい名前と不正な名前を混ぜたとき Err になり、正しいほうも置かれていないこと
- 既存テスト（install / conflict / force 周り）はそのまま通ること。`validates_agent_skill_names` の近くに、大文字や `.` を含む plugin 名風の名前（`Cool.Plugin_1`）が入ることも 1 件確認

## 作業手順

1. worktree を作る（PR #29 のルール。メインチェックアウトは main のまま）:
   `git worktree add -b fix/issue14-skill-name-path-check /home/ubuntu/projects/mesh-llm-worktrees/fix-issue14-skill-name-path-check origin/main`
2. 上記の実装とテストを書く
3. 検証（Cargo は直列で実行）:
   - `cargo fmt -p mesh-llm-skills` / `cargo fmt --all --check`
   - `cargo check -p mesh-llm-skills`、`cargo clippy -p mesh-llm-skills --all-targets -- -D warnings`
   - `cargo xtool repository affected-crates mesh/crates/mesh-llm-skills/src/lib.rs` で CI の対象 crate を出し、それぞれ `cargo test -p <crate>`（少なくとも mesh-llm-skills と mesh-llm-plugin-manager）
   - `just no-console-print`
   - `just hooks-install` 済みを確認し、コミット後 `just check-commits`
4. コミット: `fix(skills): reject skill names that are not a single path component`（Conventional Commits、**Co-Authored-By トレーラーは付けない**、本文に `Fixes #14`）
5. push して fork の main 向けに PR を作る（`gh pr create --repo miminashi/mesh-llm --base main`）。タイトルはコミットと同じ。本文は日本語で、問題・修正内容・範囲外の点・検証結果を書き、末尾に Claude Code の行
6. CI の結果を確認する
7. レポートを `/home/ubuntu/projects/mesh-llm/report/<JST タイムスタンプ>_issue14_skill_name_path_check_fix.md` に書き、このプランを `attachment/<同名>/plan.md` に添付する（コード変更のみなので PNG なし）

## 検証で確かめること

- 新テストで、不正な名前のとき Skill root の外にも中にも何も作られない（dry-run 含む）
- 既存テストが全部通る（正しい名前の install / conflict / force の動作が変わっていない）
- clippy・fmt・no-console-print・commit 規約が通り、PR の CI が green
