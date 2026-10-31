# issue #23: compute-changes の固定 EOF 区切りを直す

## Context

`.github/actions/compute-changes/derive-outputs.sh` は、変更 path の一覧を `changed_files<<EOF` … `EOF` という固定の区切りで `$GITHUB_OUTPUT` に書いている。一覧に `EOF` という path があると値がそこで閉じ、残りの path が `name=value` 行として解釈される（step が形式エラーで落ちるか、output が偽造される）。upstream にも jimoto-no-llm（#451）にも修正 PR は無いので、この fork で直して PR を出す。

## 方針

区切りを毎回ランダムに作る（GitHub Docs が勧める方法）。`changed_files` の output は残す。12 本の workflow が `steps.changes.outputs.changed_files` を `plan-ci` に渡しており、`plan-ci` 側はこの input を使っていない（`/tmp/changed_files.txt` を読んでいる）。それでも output を消すと変更が 14 ファイルに広がるので、今回は区切りだけを直す。

## 変更内容

### 1. `.github/actions/compute-changes/derive-outputs.sh`（426〜429 行の出力ブロック）

- `/dev/urandom` から 16 バイト読み、区切りを作る: `delim="ghadelim_$(od -An -tx1 -N16 /dev/urandom | tr -d ' \n')"`（openssl に依存しない）
- 値の中身は、`/tmp/changed_files.txt` を直接 `cat` するのをやめ、すでに 21 行目で作っている `$CHANGED_FILES`（`__force_all__` を除いたもの）を使う。中身が空でなければ `printf '%s\n' "$CHANGED_FILES"` で書く。これでテストから `/tmp` に触れずに出力ブロックを動かせる
- 念のため、区切りと同じ行が値の中にあれば `exit 1` で止める（`grep -qxF`）

### 2. テスト: `tools/xtask/tests/migration_lifecycle/source_layout_action/output_framing.rs`（新規）

- `source_layout_action.rs` に `#[path = ...] mod output_framing;` を足す
- `classification.rs` と同じやり方を使う。`include_str!` でスクリプトを読み、`# Set outputs using GITHUB_OUTPUT` からファイル末尾までを切り出し、`workflows::bash` で実行する。出力ブロックが参照する変数はすべて env で渡す（`bash` ヘルパーは `-u` 付きで実行するため）
- テストの中で、GitHub の output ファイルの書式（`name=value` と `name<<delim` … `delim`）どおりに出力を解析する小さなパーサを書く。形式エラーと重複キーでは panic させる
- テストケース:
  - `CHANGED_FILES` が `EOF` / `all_rust=forged` / `x<<EOF` / `normal.rs` の 4 行のとき、`changed_files` にこの 4 行がそのまま入り、`all_rust` がスクリプト側の値になる
  - `CHANGED_FILES` が空のとき、`changed_files` が空文字になり、ほかの output も揃っている

### 3. 作業手順

1. `fix/compute-changes-output-delimiter` ブランチを main から切る
2. 上の変更を入れ、`cargo test -p xtask --test migration_lifecycle output_framing` を通す（`just` に専用のレシピがあればそちらを使う）
3. 修正前のスクリプトに対してテストが落ちることも一度確認する
4. コミットする。メモリの指示どおり Co-Authored-By トレーラーは付けない
5. push して `miminashi/mesh-llm` の main 向けに PR を作る（本文に `Fixes #23`）
6. CLAUDE.md の規則に従ってレポートを `report/` に書き、plan.md を添付する。レポートは別コミットで main に入れる（既存のレポートと同じ運用）

## 検証

- 新しいテストが通り、修正前のスクリプトでは落ちる
- `bash -n derive-outputs.sh` が通る
- 既存の `source_layout_action::classification` のテストも通る（同じファイルを切り出して使っているため）
- PR を出したあと、fork の CI（pr_linux など）が compute-changes の step を通ることを `gh pr checks` で確認する
