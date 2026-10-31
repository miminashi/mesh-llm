# CI の変更一覧出力の区切りを毎回ランダムにする修正

- **実施日時**: 2026年10月31日 21:55 〜 23:25 JST (issue #23 の解説コメント投稿、upstream と jimoto-no-llm の対応状況の確認、修正とテストの作成、fork への PR 作成、fork CI で xtask のテストが落ちる問題の修正とマージ、PR の CI 再実行)
- **報告日時**: 2026年10月31日 23:25 JST
- **作成者**: Claude Opus 5.5

## 概要

CI の最初に動く「変更されたファイルを調べる」処理は、調べた一覧を後の処理に渡すとき、`EOF` という決まった文字列を区切りに使っていました。PR に `EOF` という名前のファイルが含まれていると、一覧はそこで終わったものと扱われ、残りのファイル名が後の処理への命令として読まれてしまいます。悪意がなくても CI が落ち、ファイル名を工夫すれば別の出力値を書き換えられる可能性もあります。fork の issue #23 でこの問題を扱っていました。

まず、issue を読む人向けに、仕組みと危険度を平易に解説したコメントを issue に投稿しました。区切り文字と同じ中身が来るとデータが命令に化けるという、SQL インジェクションと同じ種類のバグであることを説明しました。この fork では PR を出す人がもともと CI のスクリプト自体を書き換えられるので、実際の影響は主に「CI が落ちる」程度であることも書いています。

次に、upstream と、この問題を最初に報告した jimoto-no-llm の fork で修正が提案されていないかを調べました。どちらの最新のコードも固定の区切りのままで、修正する PR も見つかりませんでした。jimoto-no-llm 側の issue は、CI の判定を正確にする親 issue の子項目として、手つかずのまま残っています。

修正では、区切りの文字列を毎回乱数から作るようにしました。GitHub のドキュメントが勧めている方法です。万一、一覧の中に同じ文字列の行があれば、書き込まずにエラーで止めます。変更一覧の値を後の処理に渡す仕組みは、ほかの多数の workflow が使っているので、そのまま残しました。

テストは、出力を書く部分だけを切り出して実行し、GitHub の runner と同じ規則で読み直すものを新しく書きました。`EOF` や `name=value` の形をしたファイル名を混ぜても、それらがすべて一覧の値に収まり、ほかの出力が書き換えられないことを確かめています。修正前の出力部分に同じ入力を与えると、一覧が先頭で途切れ、ファイル名が命令の行として出てくることも確認しました。

修正は fork の main 向けの PR として出しました。fork では upstream 向けの CI レーンを止めてあるので、変更したスクリプトそのものは fork の CI では動きません。fork 用のチェックのうち、テストを足したツールのクレート全体のテストだけが失敗しました。原因は、CI のマシンに必要なコマンドが入っていないことなどで、今回の変更とは関係のない既存のテストです。このクレートは普段の main の CI では対象にならないため、これまで表に出ていなかったと考えられます。そこで、CI の設定を別の PR で直しました。このツールのテストは、必要なコマンドを用意したうえで 1 本ずつ順番に流すようにしています。あわせて、CI のスクリプトを検査するテスト群も CI で回るようにしました。その PR をマージしてから修正の PR を回し直すと、すべてのチェックが通り、今回足したテストも CI の上で実行されました。修正の PR のマージは、ユーザーの判断を待っています。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/plan.md)
- [修正パッチ（fork の main との差分）](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/fix.patch)
- [issue #23 に投稿した解説コメント](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/issue23_comment.md)
- [PR #26 の本文](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/pr26_body.md)
- [修正前後の出力ブロックに同じ入力を与えた結果](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/delimiter_demo.txt)
- [fork CI の失敗の抜粋](attachment/2026-10-31_222808_issue23_compute_changes_output_delimiter_fix/fork_ci_failures.txt)

## 核心発見サマリ

**結論**: upstream（Mesh-LLM/mesh-llm main `c86f58e58`）も jimoto-no-llm/mesh-llm main も、`derive-outputs.sh:427` は固定の `changed_files<<EOF` のままで、修正 PR は無い（jimoto-no-llm#451 は親 #132 の子項目として OPEN）。fork では区切りを `ghadelim_<16 バイトの乱数の hex>` に変える修正を [miminashi/mesh-llm#26](https://github.com/miminashi/mesh-llm/pull/26)（Fixes #23、コミット `be45920b1`）として出した。

- 修正前: 入力 `EOF` / `all_rust=forged` / `normal.rs` → `changed_files<<EOF`、`EOF`、`all_rust=forged`、`normal.rs`、`EOF` と出力され、値は 1 行目で閉じる
- 修正後: 3 行とも `changed_files` の値に入り、区切りの行は乱数の文字列になる
- 新しいテスト `source_layout_action::output_framing` 2 件が通る。`source_layout_action` 全体では 23 passed / 1 failed で、失敗した 1 件（`workflows::website_just_evaluation_...`）は作業環境に `just` が無いことが原因。main でも同じように失敗する
- fork の CI（Fork CI run `41205649024`）: `Rust format`、`Commit convention` などは pass。`Clippy and tests` は `cargo test -p xtask` の lib テストで 2380 passed / 4 failed。3 件は `agent_fixture_inputs::coding::tests::*` で `fixture test requires just`、1 件は `ui_build::execution_tests::actual_deadline_cleans_native_descendant_and_leaves_no_success_stamp` で `writer.txt` が無い（`NotFound`）。どれも今回変えていないコード。lib テストで止まったため、新しく追加した `migration_lifecycle` のテストは CI では実行されていない
- fork CI の修正: [miminashi/mesh-llm#27](https://github.com/miminashi/mesh-llm/pull/27)（merge commit `77ade8fdc`）。xtask が対象のときは Node 24.20.0 と just を入れ、`cargo test -p xtask -- --test-threads=1` と `--features automation-contracts --test migration_lifecycle source_layout_action` を流す。#27 自身の CI で xtask は 2384 passed / 13 ignored、`source_layout_action` は 22 passed
- #26 を main に追従させた後の CI（run `41209260452`、head `d3b37e661`）: 全チェックが pass。xtask は 2384 passed、`source_layout_action` は 24 passed で、`output_framing` の 2 件も CI で ok

## 前提・目的

- **背景**: [miminashi/mesh-llm#23](https://github.com/miminashi/mesh-llm/issues/23)（元は jimoto-no-llm/mesh-llm#451）。`.github/actions/compute-changes/derive-outputs.sh` が `git diff --name-only` の結果を、固定の `EOF` の区切りで `$GITHUB_OUTPUT` に書いていた
- **目的**: fork の main に修正を入れる。upstream には出さない（fork は自分たち用のセキュリティ修正のためのもの）
- **影響範囲の見立て**: `compute-changes` を使う PR 用 workflow は `pull_request` トリガーで動き、PR 側の workflow とスクリプトが使われる。fork からの PR には secret も書き込み権限も渡らない。そのため、新しく得られる権限はほぼなく、実際の影響は CI の失敗と出力の汚染にとどまる

## 環境情報

- ワークステーション、Rust stable 1.99.1
- base: fork の main `197e8c954`（upstream main `c86f58e58` を含む）
- ブランチ `fix/compute-changes-output-delimiter`、HEAD `be45920b1`
- `.cargo/config.toml` は `rustc-wrapper = "sccache"` を指定しているが、ワークステーションに sccache が無いため `RUSTC_WRAPPER=` を付けて実行した

## 再現方法

```bash
export RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER=
cargo test --locked -p xtask --features automation-contracts \
  --test migration_lifecycle source_layout_action::output_framing
```

修正前の挙動は、main の `derive-outputs.sh` から `# Set outputs using GITHUB_OUTPUT` 以降を切り出し、`/tmp/changed_files.txt` を `EOF` を含むファイルに差し替えて実行すると再現できる（結果は添付の `delimiter_demo.txt`）。

## 結果詳細

### 上流と jimoto-no-llm の対応状況（2026-10-31 時点）

- upstream: `derive-outputs.sh` の `changed_files<<EOF` は main でも 427 行目に残る。fork が取り込んだ `007d052d8` 以降、`compute-changes` を変えたコミットは無い。`derive-outputs`、`compute-changes`、`changed_files<<EOF`、`GITHUB_OUTPUT delimiter` などで PR と issue を検索したが、該当するものは無い（#2307 が全文検索で当たったが、`.github` 配下を変更しておらず無関係）
- jimoto-no-llm: #451 は OPEN、コメントも紐づく PR も無い。親 #132「CIの変更対象判定と実行計画を正確にする」の子項目（#449〜#453）はすべて未チェック。main の該当ファイルの最終変更は 2026-10-05（#2099）

### 変更内容

- `derive-outputs.sh`
  - `od -An -tx1 -N16 /dev/urandom` で区切りを作る（openssl に依存しない）
  - 区切りが空になった場合と、一覧に区切りと同じ行がある場合は `exit 1`
  - 一覧は `/tmp/changed_files.txt` を読み直さず、21 行目で `__force_all__` を除いて作った `$CHANGED_FILES` から書く。中身は同じで、テストから `/tmp` に触れずに出力ブロックを動かせる
- `tools/xtask/tests/migration_lifecycle/source_layout_action/output_framing.rs`（新規）
  - 出力ブロックを `include_str!` で切り出し、既存の `workflows::bash` ヘルパー（`-euo pipefail`）で実行する
  - runner の書式（`name=value`、`name<<delim` … `delim`）で解析し、形式エラーと重複キーでは panic する
  - `EOF` / `all_rust=forged` / `x<<EOF` / `normal.rs` を含む一覧と、空の一覧の 2 ケースで、全 18 個の output が期待どおりになることを確かめる
- `changed_files` の output は残した。12 本の workflow が `plan-ci` の `changed_files` input に渡しているが、`plan-ci` 側は使わずに `/tmp/changed_files.txt` を読んでいる

### テスト

- 修正前の main のスクリプトに対しては、新しいテストのうち `changed_paths_that_look_like_output_commands_stay_inside_changed_files` が失敗した。ただし原因は、旧ブロックが `/tmp/changed_files.txt` を直接読むため、`changed_files` が空になったこと。区切りの衝突そのものは、上の `delimiter_demo.txt` で確認した
- 修正後: `source_layout_action` 24 件中 23 件が通過。`classification` の 2 件（同じスクリプトを切り出して使う）も通過。失敗した 1 件は `just` が無いことによるもので、main でも失敗する

## 副次発見

- `fork-ci.yml` は変更されたクレートだけを `cargo test -p` する。`tools/xtask` を変更すると xtask が対象になり、runner に `just` が無いことと並列実行のせいで lib テストが 4 件落ちていた。main の push では xtask が選ばれていなかったので、これまで表に出ていなかった（#27 で修正済み）
- fork の CI は PR #10 で upstream 向けのレーンを skip するようにしてあるため、`compute-changes` は fork の PR では動かない。今回の修正は、ローカルのテストと手動の再現でしか確かめていない
- `plan-ci` の `changed_files` input は使われておらず（説明文も "Bounded compatibility input; the planner reads the generated file"）、`compute-changes` の `changed_files` output も実質的には不要。消せば区切りの問題そのものがなくなるが、14 ファイルの変更になるので今回は見送った

## 残課題

- PR #26 のマージ（CI はすべて pass）
- 必要なら、使われていない `changed_files` の output と `plan-ci` の input をまとめて取り除く
- jimoto-no-llm の親 issue #132 のほかの子項目（#449、#450、#452、#453）が、この fork にも当てはまるかの確認
