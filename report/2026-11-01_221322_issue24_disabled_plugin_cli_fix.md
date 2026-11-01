# config で無効にした plugin を CLI から起動できない修正

- **実施日時**: 2026年11月1日 21:20 〜 22:15 JST (issue #24 の解説コメント、upstream / jimoto-no-llm の修正状況の確認、CLI 解決処理の修正とテスト)
- **報告日時**: 2026年11月1日 22:15 JST
- **作成者**: Claude Opus 5.5

## 概要

issue #24 は、config ファイルで `enabled = false` にした plugin でも、`mesh-llm <plugin名> ...` という外部 CLI コマンドの形で呼べば起動できてしまう問題です。plugin を止めるつもりで config に書いても、CLI の経路だけ黙って無視されていました。

まず issue に噛み砕いた解説をコメントし、upstream（Mesh-LLM/mesh-llm）と jimoto-no-llm/mesh-llm で修正が提案されていないかを確認しました。どちらにも修正の PR や関連する議論は無く、upstream の main にも問題のコードがそのまま残っていました。

原因は、CLI の解決処理が「config で無効」を「config に無い」と同じ値（`None`）で返していたことです。呼び出し側はそれを「見つからなかった」と受け取り、installed store を探し直して、そこで有効になっている同じ名前の plugin を起動していました。`serve` で plugin を立ち上げる通常の経路は、config にある名前を installed store 側で飛ばすので、この問題は起きません。

修正では、config に同じ名前の entry があって無効になっていれば、その場でエラーを返し、installed store や bundled への fallback に進まないようにしました。エラーメッセージで「config で無効になっている」と分かるようにしています。config に無く、installed store で有効な plugin は、これまでどおり CLI から使えます。

回帰テストを 1 本追加しました。修正を外すとこのテストだけが失敗し、修正を入れると既存のテストも含めてすべて通ることを確認しています。

## 前提・目的

- **背景**: plugin の有効/無効は、config の `[[plugin]]` entry と installed store の metadata の 2 か所に書かれる。両方に同じ名前があるときは config が優先される、というのが runtime 側（`append_installed_plugins`）のルール
- **目的**: CLI の解決処理を runtime 側のルールに揃え、config で無効にした名前が別の経路で起動しないようにする

## 環境情報

- ワークステーション（ビルド・テストのみ。GPU サーバは使っていない）
- ブランチ: `fix/issue24-disabled-plugin-cli`（`origin/main` = `591e7d015` から作成）
- 修正コミット: `863c5498e`

## 結果詳細

### 修正内容

`mesh/crates/mesh-llm/src/commands/plugin_cli.rs` の `resolve_configured_cli_plugin`:

- 変更前: config の entry が `enabled = false` なら `plugin::bundled_cli_plugin_spec(command)` を返す。この関数は常に `Ok(None)` を返すため、呼び出し元の `resolve_required_cli_plugin` が続けて `resolve_installed_cli_plugin` を呼び、store 側で有効な実行ファイルを起動していた
- 変更後: `bail!("Plugin command '{command}' is disabled in the mesh-llm config")` でエラーを返す

監査用のコマンド分類 `external_cli_command_family` も同じ関数を使っている。無効な plugin は、これまでの `Unknown` ではなく `Plugin` に分類されるようになる。分類値は有限の enum で、コマンド名や引数は外に出ないので、問題はない。

### テスト

追加: `config_disabled_plugin_does_not_fall_back_to_installed_store`

- installed store に `demo`（`enabled = true`、実行ファイルあり）を置き、config に `[[plugin]] name = "demo" enabled = false` を書く
- `resolve_required_cli_plugin` がエラーになり、メッセージに `disabled in the mesh-llm config` を含むことを確認する
- `external_cli_command_family` が `Plugin` を返すことを確認する

| 状態 | `cargo test -p mesh-llm --lib plugin_cli` |
|---|---|
| 修正を外した状態 | 新しいテストだけ FAILED（`expect_err` で panic。installed store の plugin に解決された）、既存 3 本は ok |
| 修正あり | 4 passed |

### 検証

- `cargo test -j 8 -p mesh-llm --lib plugin_cli`: 4 passed
- `cargo clippy -j 8 --locked -p mesh-llm --all-targets -- -D warnings`: 警告なし
- `rustfmt --edition 2024 --check`: 差分なし

### upstream / jimoto-no-llm の状況（2026-11-01 確認）

- `upstream/main` の `plugin_cli.rs` / `config.rs` は、この fork の main と同じで、問題のコードが残っている
- Mesh-LLM/mesh-llm で `plugin_cli.rs` に触れている open PR は #2702 だけで、`endpoint_models: None` を足す 1 行の変更（この問題とは関係ない）
- jimoto-no-llm/mesh-llm#269 は open のまま。コメントも、紐づく PR も無い

## 残課題

- `resolve_required_cli_plugin` は、`resolve_configured_cli_plugin` が `None` を返したあとに `resolve_installed_cli_plugin` をもう一度呼んでいる。前者の中ですでに同じ検索をしているので重複しているが、動作には影響しないため今回は触っていない
- 実際のバイナリで `mesh-llm <plugin名>` を実行する確認はしていない（単体テストで解決処理を確認しただけ）
