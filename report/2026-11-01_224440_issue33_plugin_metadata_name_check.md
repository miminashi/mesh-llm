# plugin 台帳の名前とフォルダ名の照合を追加

- **実施日時**: 2026年11月1日 22:00 〜 22:45 JST (issue #33 の解説コメントと他リポジトリの確認、名前の照合の実装、テスト追加)
- **報告日時**: 2026年11月1日 22:45 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm は、インストールした plugin ごとにフォルダを作り、その中の台帳ファイルに plugin の名前や置き場所を記録しています。一覧を作るときはフォルダ名を検査していましたが、プログラムが実際に使うのは台帳の中に書かれた名前で、こちらは検査もフォルダ名との照合もしていませんでした（issue #33）。台帳を書き換えると、検査を通っていない名前が skill の登録や plugin の起動に入り込みます。

作業ではまず、issue #33 の内容を噛み砕いた解説を issue にコメントしました。そのうえで、upstream と jimoto-no-llm/mesh-llm に修正提案がないかを確かめました。どちらにも #33 の修正はありませんでした。jimoto 側には、壊れた台帳が 1 件あるだけで一覧全体が失敗する、という近い issue がありました。直し方の方向が同じなので、その参照も #33 にコメントしました。

修正では、台帳を読み込む入口で、台帳の名前とフォルダ名が一致するかを確かめるようにしました。名前を指定して 1 つの plugin を読むときは、一致しなければエラーにします。一覧を作るときは、一致しない plugin だけを外し、残りの plugin はこれまでどおり使えるようにしました。外した plugin は、起動時の plugin 一覧に、フォルダ名のままエラーとして表示します。

この照合は、issue で挙げた skill 名の問題だけでなく、調べる途中で見つけた次の問題もまとめて止めます。台帳に別の plugin の名前を書いてその plugin になりすますこと。有効・無効の切り替えで、別の plugin の台帳を上書きしてしまうこと。plugin の削除で、台帳に書かれた任意のフォルダを消させること。照合に失敗した台帳はどの操作にも使われないので、削除では plugin のフォルダだけが消え、台帳に書かれた置き場所は消えません。

テストは、名前の合わない台帳について、読み込みの拒否、一覧からの除外と報告、削除時の振る舞い、起動時の扱いの 4 件を足しました。既存のテストと合わせてすべて通り、clippy も警告なしで通りました。

残る課題は、台帳の中身そのもの（壊れた JSON）で一覧全体が失敗する問題です。これは jimoto 側の issue が扱っている内容で、今回は手を付けていません。

## 添付ファイル

- [host-runtime の plugin テスト結果](attachment/2026-11-01_224440_issue33_plugin_metadata_name_check/host-runtime-plugin-tests.log)
- [新しいテストと clippy の結果](attachment/2026-11-01_224440_issue33_plugin_metadata_name_check/new-tests-and-clippy.log)

## 核心発見サマリ

**結論**: `PluginStore::try_load` で `metadata.name != <ディレクトリ名>` ならエラーにしました。`list()` は不一致の項目を飛ばし、新しく足した `list_checked()` はそれを `rejected` として返します。host-runtime の `append_installed_plugins` は、`rejected` をディレクトリ名・`status=error` の inactive summary にして起動しません。`load` / `set_enabled` は不一致でエラーになります。`delete` は `load(name).ok()` が `None` になるので、台帳の `install_path` を消さずに store のディレクトリだけを消します。テストは、`cargo test -p mesh-llm-plugin-manager` が 80 件、`cargo test -p mesh-llm-host-runtime --lib plugin::` が 369 件すべて通り、clippy（2 crate、`--all-targets -D warnings`）も通りました。

## 前提・目的

- **背景**: issue #33（Low）。#14（PR #30）の修正中に見つかった。#30 で `install_skills` の入口に検査を足したため、Skill root の外に書き込むことはもうできない。ただし、検査を通っていない名前が他の処理に入ること自体は残っていた
- **目的**: 台帳の `name` を、検査済みのディレクトリ名と必ず一致させる。1 件の不一致で、ほかの plugin まで使えなくならないようにする
- **他リポジトリの状況（2026-11-01 確認）**: upstream（Mesh-LLM/mesh-llm）に該当する issue・PR はなく、`upstream/main` の `store.rs` も照合なし。jimoto-no-llm/mesh-llm には #33 の修正はないが、近い issue として jimoto-no-llm/mesh-llm#209（壊れた metadata があっても一覧を返す。open、PR なし）がある

## 環境情報

- ブランチ: `fix/issue33-plugin-metadata-name`（`origin/main` の `591e7d015` から）
- 修正コミット: `4fceb9184`
- 実行環境: ワークステーション（lld でリンク、`-j 8`）

## 結果詳細

### 変更内容

| ファイル | 変更 |
|---|---|
| `mesh/crates/mesh-llm-plugin-manager/src/store.rs` | `try_load` を、照合なしで読む `read_metadata` と、照合する `name_mismatch` に分けた。`list()` は `list_checked().plugins` を返すようにし、`PluginListing` / `RejectedPluginMetadata` を追加した |
| `mesh/crates/mesh-llm-plugin-manager/src/lib.rs` | 新しい 2 つの型を再 export |
| `mesh/crates/mesh-llm-host-runtime/src/plugin/installed.rs` | `list_checked()` を使い、`rejected` をエラー行（`rejected_installed_plugin_summary`）として inactive に積む |

エラーメッセージの例:

```
plugin metadata <root>/demo/plugin-install.json names plugin "../../escape", but it is stored under "demo"
```

`{:?}` で出すので、改行などの制御文字を含む名前でも、表示が崩れません。

### 不一致の台帳に対する各操作の振る舞い

| 操作 | 修正前 | 修正後 |
|---|---|---|
| `list()`（skill の検出、`plugins list`、Web UI の状態取得など） | 台帳の `name` をそのまま返す | その項目を外す |
| 起動（`append_installed_plugins`） | 台帳の `name` で起動する。同じ名前が先にあれば、どちらかが起動しない | 起動しない。ディレクトリ名でエラー行を出す |
| `load` / `try_load` / `load_optional` | 台帳をそのまま返す | エラー |
| `set_enabled` | 台帳の `name` 側のディレクトリに書き戻す | エラー（何も書かない） |
| `delete` | 台帳の `install_path` を `remove_dir_all` する | `install_path` は消さず、store のディレクトリだけを消す |

### 追加したテスト

- `store::tests::load_rejects_metadata_naming_another_plugin`: `name` を `../../escape` に書き換えた台帳で、`load` / `try_load` / `set_enabled` がエラーになる
- `store::tests::list_skips_and_reports_metadata_naming_another_plugin`: `../../escape` と、別の正しい plugin 名（`blackboard`）に書き換えた 2 件が、`list()` から外れ、`list_checked().rejected` に入る。正しい plugin は残る
- `store::tests::delete_of_mismatched_metadata_keeps_its_install_path`: 不一致の台帳を `delete` しても、台帳に書かれた `install_path` は残る
- `plugin::installed::tests::metadata_naming_another_plugin_is_reported_not_started`: `capsules` を名乗る別ディレクトリの台帳は起動されず、ディレクトリ名でエラー行になる。本物の `capsules` は起動される

新しいテストは、修正前のコードでは試していません。`list_checked` が修正前にはなくコンパイルできないためです。`load` のテストは、修正前の `try_load` が台帳をそのまま返すことから、失敗することがコードから分かります。

## 副次発見

- issue #33 の本文には、`plugin.name` の他の使い道がどうなるかは「確かめていない」と書いていた。今回調べると、`set_enabled` による別 plugin の台帳の上書きと、`delete` による `install_path` の削除が見つかった。どちらも今回の照合で止まる。ただし、`install_path` が plugin のインストール先の下にあるかは、今回も確かめていない（名前の一致した台帳の `install_path` を書き換えることは、引き続きできる）。前提は「store に書き込める人」で同じなので、深刻度も同じ

## 残課題

- 壊れた JSON の台帳が 1 件あると、`list()` 全体が失敗する（jimoto-no-llm/mesh-llm#209 と同じ問題）。今回の `list_checked` の `rejected` に、parse の失敗も入れれば直せる
- `install_path` が store のインストール先の下にあるかの検査（上の副次発見）。必要なら別 issue にする
