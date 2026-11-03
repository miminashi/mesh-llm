# plugin 台帳の install_path を検査し、delete で任意のディレクトリが消えないようにする

- **実施日時**: 2026年11月2日 00:40 〜 05:07 JST (issue #54 の解説と他リポジトリの確認、install_path の照合と delete の修正、テスト・ミューテーション確認)
- **報告日時**: 2026年11月2日 05:07 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm に入れた plugin には、それぞれ「台帳」にあたるファイルがあり、そこに plugin 本体の置き場所（`install_path`）が書かれています。mesh-llm はこの値を確かめずに使っていました。特に `mesh-llm plugins delete` は、書かれた場所を中身ごと消します。台帳を書き換えられると、利用者が plugin を削除したときに、ホームディレクトリなど任意のディレクトリを消させられる状態でした（issue #54）。

修正に入る前に、issue へ噛み砕いた解説を書きました。また、upstream と jimoto-no-llm/mesh-llm に修正提案が無いことも確認し、その結果も issue に残しました。

修正では、守りを 2 段にしました。1 段目として、台帳を読むときに「本体の置き場所が、mesh-llm 自身がインストールするときに使う場所とちょうど同じか」を確かめます。違う台帳は、名前が合わない台帳（#50）と同じように扱います。名前を指定して読むときはエラーにし、一覧を作るときはその台帳だけを外して報告します。台帳を書くときも同じ規則で確かめるので、mesh-llm 自身が合わない台帳を書くことはありません。2 段目として、削除は台帳の値を使わず、plugin の名前から決まる場所だけを消すようにしました。

「その下にあればよい」ではなく「ちょうどその場所」で比べるのは、全 plugin の本体置き場そのものや、別の plugin の本体を指す台帳も拒否するためです。比べ方は、ファイルシステムを見に行かず、書かれたパスを要素ごとに比べる方式にしました。そのため、`..` を含む書き方なども拒否されます。

あわせて、インストール先を store の外に指定した場合は、展開の前に拒否するようにしました。本番の経路ではインストール先は常に store の下なので、影響を受けるのはテストだけです。インストール先を store の外に置いていた既存のテストは、本番と同じ配置に直しました。

新しいテストでは、台帳が store の外・store 自体・本体置き場そのもの・別の plugin・`..` を含む書き方などを指していても、読み込みで拒否されること、削除でそこが消えないことを確かめています。修正を外すと、新しいテストが失敗することも確認しました。

関係する crate のテストは、plugin と関係のない sdk のテスト 1 件を除いて通っています。この 1 件は、修正前のコードでも同じように失敗します。

並行して進んでいる #55（名前が `installed` の plugin の問題、PR #57）とは同じファイルを触っています。先にマージされたほうに合わせて、もう一方を調整する必要があります。

## 添付ファイル

- [plugin-manager のテスト結果](attachment/2026-11-02_050654_issue54_plugin_install_path_check/plugin-manager-tests.log)
- [clippy の結果（`-A clippy::too_many_lines`）](attachment/2026-11-02_050654_issue54_plugin_install_path_check/clippy.log)
- [4 crate のテスト結果の要約](attachment/2026-11-02_050654_issue54_plugin_install_path_check/host-tests-summary.log)
- [ミューテーション確認に使ったスクリプト](attachment/2026-11-02_050654_issue54_plugin_install_path_check/mutate.py)

## 核心発見サマリ

**結論**: `PluginStore` の `try_load` / `list_checked` / `save` で、`install_path == <store root>/installed/<name>` を `Path` の等値（要素ごとの比較、`canonicalize` しない）で確かめるようにした。`delete` は台帳を見ずに `<store root>/installed/<name>` を消す。そこがシンボリックリンクなら、リンクだけを消す。`extract_for_install` は、`<store root>/installed` 以外のインストール先を展開前に拒否する。`cargo test -p mesh-llm-plugin-manager` は 88 件すべて通る。ミューテーション確認では、照合を外す・「下にあればよい」に緩める・インストール先の検査を外す、のいずれでも新しいテストが失敗した。照合と `delete` の両方を元に戻すと、delete のテストも失敗した。`delete` だけを元に戻した場合は、照合が台帳を先に拒否するので、テストは通る（2 段の守りのどちらか一方で防げる）。

## 前提・目的

- **背景**: issue #54。`PluginStore::delete` は、`load` した台帳の `install_path` を `remove_dir_all` していた。起動（`executable_path`）、Web UI（`web_ui_asset_root_path`）、skill の検出（`skills.rs`）も、同じ値を使う
- **目的**: `install_path` を store の規則と照合し、`delete` が台帳に引きずられないようにする
- **前提条件**: upstream（`8d7035a10`）と jimoto-no-llm/mesh-llm に修正提案は無い（[issue コメント](https://github.com/miminashi/mesh-llm/issues/54#issuecomment-6560355228)）。解説は[こちらのコメント](https://github.com/miminashi/mesh-llm/issues/54#issuecomment-6560229911)

## 環境情報

- ワークステーション（rustup 同梱の lld でリンク、`-j 8`）
- ブランチ: `fix/issue54-plugin-install-path`（`origin/main` `39d480ebc` から）
- 修正コミット: `aed938ddc`

## 結果詳細

### 変更内容

| ファイル | 変更 |
|---|---|
| `mesh-llm-plugin-manager/src/store.rs` | `INSTALL_DIR_NAME`、`PluginStore::install_root` / `install_path` を追加。`metadata_mismatch`（名前の照合と `install_path_mismatch`）を `try_load` と `list_checked` で使う。`save` も `install_path_mismatch` で拒否する。`delete` は `install_path(name)` を `symlink_metadata` で見て消す（ディレクトリなら `remove_dir_all`、それ以外は `remove_file`、Windows のディレクトリリンク向けに `remove_dir` へ fallback） |
| `mesh-llm-plugin-manager/src/install.rs` | `extract_for_install` で、`options.install_root` が `PluginStore::install_root()` と違えば拒否する。`from_env` は `INSTALL_DIR_NAME` を使う |
| `mesh-llm-plugin-manager/src/{bundled,install,skills}.rs` のテスト | `store_root = <tmp>/store`、`install_root = <tmp>/installed` だったものを `<tmp>/store/installed` に直した |
| `mesh-llm-host-runtime` のテスト 5 ファイル | 台帳を `/tmp/...` や `<store>/installs/...` に置いていたものを、`<store>/installed/<名前>` に直した |

### 判断したこと

- **文字列での比較（`canonicalize` しない）**: 読み込みのたびにファイルシステムへ問い合わせず、本体が無いときにも同じ判定になる。`..` や `.` を含む書き方も拒否される。代わりに、インストール後に store root の書き方が変わると（`MESH_LLM_PLUGIN_DIR` の書き方を変えた、`$HOME` がシンボリックリンク経由になった、など）、その plugin は拒否される。`plugins delete` で消してから入れ直せば直る
- **拒否された plugin の `install`**: `install` は既存の台帳を `load_optional` するので、拒否された台帳があるとエラーになる。#50 の名前の不一致と同じ扱い。`delete` は台帳を読まずに消すので、`delete` → `install` で復旧できる
- **`PluginInstallOptions::install_root` は残した**: upstream と同じ形を保ち、取り込みのときの衝突を減らすため。かわりに、`<store root>/installed` 以外を展開の前に拒否する
- **`delete` は台帳が無くても `installed/<名前>` を消す**: これまでは台帳が読めないと本体を残していた。今は名前から決まる場所を消すので、台帳が壊れていても本体が残らない

### 追加したテスト（`store.rs` 6 本、`install.rs` 1 本）

| テスト | 確かめること |
|---|---|
| `load_and_save_reject_an_install_path_other_than_the_plugins_own` | 外部ディレクトリ、store root、`installed` そのもの、別の plugin（`installed/notes`）、自分の下（`installed/demo/nested`）、`installed/other/../demo`、`<root>/../<root 名>/installed/demo`、相対パス `installed/demo`。いずれも `load` / `try_load` / `set_enabled` / `save` で拒否される |
| `list_skips_and_reports_metadata_placing_files_elsewhere` | 外部ディレクトリや別の plugin を指す台帳は `list` から外れ、`rejected` で報告される |
| `delete_never_removes_the_directory_an_edited_record_names` | 上の各パスを指す台帳を `delete` しても、そこは消えない。ほかの plugin の本体と台帳は残る |
| `delete_removes_a_link_in_place_of_the_files_not_its_target`（unix） | `installed/demo` がシンボリックリンクなら、リンクだけを消す |
| `delete_removes_the_files_of_a_plugin_without_metadata` | 台帳が無くても `installed/<名前>` を消す。`installed` 自体は残る |
| `deletes_metadata_directory`（書き換え） | 本番と同じ配置で消せる。ほかの plugin は残る |
| `install::tests::an_install_root_outside_the_store_is_refused_before_extraction` | store の外のインストール先は拒否され、何も作られない |

### テスト結果

- `cargo test -p mesh-llm-plugin-manager`: 88 passed
- `cargo test --lib -p mesh-llm-plugin-manager -p mesh-llm-commands -p mesh-llm-host-runtime -p mesh-llm`: host-runtime で 4427 passed / 1 failed、そのほかは全件 passed
  - 失敗した `sdk::tests::embedded_startup_times_out_and_cleans_up_worker` は、変更したファイルを `origin/main` に戻しても 3 回とも同じく失敗した（今回の変更とは無関係）。1 回目の全体テストでは通っている。ワークステーションに plugin store（`~/.mesh-llm/plugins`）は無い
  - 1 回目の全体テストでは、`api::routes::logs::delete::tests::accepted_delete_timeout_returns_pending_without_a_failure_audit` も落ちた。単独で流すと通るので、並列実行の負荷によるものと見ている
- `cargo clippy -p mesh-llm-plugin-manager -p mesh-llm-host-runtime --all-targets -- -D warnings`: `mesh/node.rs` の `too_many_lines`（201/200）で止まる。`node.rs` は `origin/main` と同じで、今回の変更とは無関係。`-A clippy::too_many_lines` を付けると通る

### ミューテーション確認

| ミューテーション | 結果 |
|---|---|
| `install_path_mismatch` を常に通す | `load_and_save_reject_...` と `list_skips_...` が失敗（86 passed / 2 failed） |
| 「`install_root` の下にあればよい」に緩める | 同じ 2 本が失敗（86 / 2） |
| `extract_for_install` の検査を外す | `an_install_root_outside_the_store_...` が失敗（87 / 1） |
| `delete` を台帳の `install_path` を消す形に戻す | 全件 passed。照合が台帳を先に拒否し、名前から決まる場所が消されるため |
| 上の 2 つ（照合と `delete`）を両方戻す（＝修正前の動き） | `delete_never_removes_...` を含む 3 本が失敗（85 / 3） |

## 残課題

- PR #57（#55、`installed` を予約名にする）と同じファイルを触っている。両方で同じ `INSTALL_DIR_NAME` を定義しているので、後からマージするほうで 1 つにまとめる。#57 で予約名の plugin を `delete` するときは `installed/installed` を消す。ここは今回の `install_path(name)` と同じ場所なので、両立する
- `mesh/node.rs` の `too_many_lines` と、sdk のタイムアウトのテストは main 側の問題。必要なら別 issue にする

## 参照レポート

- [plugin 名 `.` と `..` の拒否（#32）](./2026-11-01_223843_issue32_plugin_name_dot_fix.md)
- [台帳の name とディレクトリ名の照合（#33）](./2026-11-01_224440_issue33_plugin_metadata_name_check.md)
