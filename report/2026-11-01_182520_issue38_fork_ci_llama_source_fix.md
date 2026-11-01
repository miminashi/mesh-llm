# Fork CI で llama.cpp のソースを用意してからテストする修正

- **実施日時**: 2026年11月1日 18:19 〜 18:25 JST (issue #38 の原因調査、fork-ci.yml の修正、ローカルでの確認)
- **報告日時**: 2026年11月1日 18:25 JST
- **作成者**: Claude Opus 5.5

## 概要

この fork の CI（Fork CI）は、変更のあった crate だけをテストします。ただし `Cargo.lock` のような共有のファイルが変わると、ワークスペースのすべての crate をテストします。そのとき、`skippy-quantize` のテストが 2 件、必ず失敗していました。このテストは、llama.cpp のソースにある量子化の一覧表を読んで、mesh-llm 側の一覧と食い違いがないかを確かめます。しかし Fork CI は llama.cpp のソースを用意していなかったので、ファイルが無くて失敗していました。PR #36 で依存を 1 つ足したときに表に出た問題です（issue #38）。

upstream の CI は、テストの前に `scripts/prepare-llama.sh pinned` を実行しています。これは、pin した版の llama.cpp を取得してパッチを当てるスクリプトです。Fork CI にも同じ手順を入れ、`skippy-quantize` をテストするときだけ実行するようにしました。ソースを用意するだけで、ビルドはしません。

ワークステーションで同じスクリプトを実行すると、45 秒ほどで終わりました。そのあと `skippy-quantize` のテストを実行すると、失敗していた 2 件を含めてすべて通りました。

新しい手順が CI で実際に動くかは、この PR の CI で確かめます。`fork-ci.yml` を変えた PR では、試しに動かす crate の一覧に `skippy-quantize` も入るようにしたので、この PR でも新しい手順を通ります。

ワークフローの文法チェック（actionlint）は、ツールをダウンロードして実行する操作が権限で拒否されたので、ローカルでは実行していません。

## 添付ファイル

- なし（plan mode を使っていないので、実装プランは無い）

## 前提・目的

- **背景**: issue #38。PR #36（run 41223210951）の Tests ステップで、`skippy-quantize` の `types::tests::parses_every_current_llama_quant_cli_name` と `public_quant_catalog_covers_pinned_llama_quantize_table` が、`.deps/llama.cpp/tools/quantize/quantize.cpp` が無いために panic した
- **目的**: `Cargo.lock` が変わる PR（`all_rust: true`）でも、Fork CI がこの 2 件を正しく実行できるようにする

## 環境情報

- ワークステーション（Linux 6.8）。リンカーは rust-lld（rustup に同梱されているものを PATH の先頭に足した）、`RUSTC_WRAPPER=`
- ブランチ: `fix/issue38-fork-ci-llama-source`（`origin/main` = `cb2b5dbd4` から）、修正コミット `4687fb10e`
- llama.cpp の pin: upstream `620d2225cc436ab0b001e3200e590e3b286ef9ab`、パッチ後 `cf733aafb9f9ad220c3872a81a77fba9da9acfd3`

## 結果詳細

### 原因

- `skippy/crates/skippy-quantize/src/types.rs` の `pinned_llama_quant_option_names` は、リポジトリのルートからの相対パスで `.deps/llama.cpp/tools/quantize/quantize.cpp` を読む。Rust のテストで llama.cpp のソースツリーを直接読むのは、この crate だけだった（`grep '\.deps/llama' --include=*.rs` で確かめた）
- `.github/workflows/fork-ci.yml` の「Select crates」は、`all_rust` が true のとき `.affected[]` をすべて選ぶ。除外するのは、静的な llama ABI をリンクする crate だけ
- upstream の `ci-rust-tests-slice.yml` は、テストの前に「Prepare patched llama.cpp」（`scripts/prepare-llama.sh pinned`）を実行している

### 変更内容（`.github/workflows/fork-ci.yml`）

- 「Select crates」で、選んだ crate に `skippy-quantize` があれば、`llama_source=true` を出力する
- 「Tests」の前に「Prepare pinned llama.cpp source」（`scripts/prepare-llama.sh pinned`）を追加した。`llama_source == 'true'` のときだけ実行する
- `fork-ci.yml` を変えたときに試しに動かす crate の一覧に、`skippy-quantize` を追加した（新しい手順を必ず通すため）

### ローカルでの確認

- `scripts/prepare-llama.sh pinned`: EXIT=0、45 秒、`.deps/llama.cpp` は 649M
- `cargo test -j 8 --locked -p skippy-quantize`: 106 + 1 + 4 passed（ソースがあれば、失敗していた 2 件も通る）
- `git diff --check`、YAML の構文チェック（`yaml.safe_load`）: OK
- actionlint: 未実施（ダウンロードして実行する操作が権限で拒否された）
- `cargo xtool repository conventional-commits --range origin/main..HEAD`: OK

## 残課題

- clone した llama.cpp を `actions/cache` に保存すれば 45 秒ほど縮められる。今は、`skippy-quantize` を選んだときにしか実行しないので入れていない
- これから llama.cpp のソースを読むテストが他の crate に増えたら、「Select crates」の `case` にその crate を足す必要がある
