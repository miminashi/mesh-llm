## 概要

`compute-changes` は変更 path の一覧を、固定の `EOF` を区切りにして `$GITHUB_OUTPUT` に書いていました。`EOF` という名前の path があると値がそこで閉じ、残りの path が output の命令として読まれます（形式エラーで step が落ちる、または `name=value` 形式の path で別の output が書かれる）。

Fixes #23

## 変更

- `.github/actions/compute-changes/derive-outputs.sh`
  - 区切りを毎回 `/dev/urandom` の 16 バイトから作る（`ghadelim_<32 桁の hex>`）。万一、一覧の中に同じ行があれば書かずに失敗する
  - 一覧は `/tmp/changed_files.txt` を読み直さず、`__force_all__` を除いた後の `$CHANGED_FILES` から書く（中身は同じ）
- `tools/xtask/tests/migration_lifecycle/source_layout_action/output_framing.rs`（新規）
  - 出力ブロックを切り出して実行し、runner と同じ規則（`name=value` と `name<<delim` … `delim`）で解析する。形式エラーと重複キーでは失敗する
  - `EOF` / `all_rust=forged` / `x<<EOF` / `normal.rs` を含む一覧が `changed_files` にそのまま入り、ほかの output が上書きされないことを確認する
  - 一覧が空でもすべての output が書かれることを確認する

`changed_files` の output は、12 本の workflow が `plan-ci` の input に渡しているので残しました（`plan-ci` 側はこの input を使わず、ファイルを読んでいます）。

## 確認

- `cargo test --locked -p xtask --features automation-contracts --test migration_lifecycle source_layout_action`: 新しい 2 件を含め 23 件が通過
  - `workflows::website_just_evaluation_...` の 1 件は失敗するが、作業環境に `just` が無いため。main でも同じように失敗する
- 修正前の出力ブロックに `EOF` / `all_rust=forged` / `normal.rs` を入れると、`changed_files<<EOF`、`EOF`、`all_rust=forged`、`normal.rs`、`EOF` の順に出力され、値が 1 行目で閉じることを確認した。修正後は 3 行とも `changed_files` の値に入る

🤖 Generated with [Claude Code](https://claude.com/claude-code)

