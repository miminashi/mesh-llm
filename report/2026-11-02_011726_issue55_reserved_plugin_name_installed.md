# plugin 名 installed を予約名にして、ほかの plugin の消失を防ぐ

- **実施日時**: 2026年11月2日 00:45 〜 01:25 JST (issue #55 の解説・他リポジトリの確認、案 A による修正とテスト)
- **報告日時**: 2026年11月2日 01:25 JST
- **作成者**: Claude Opus 5.5

## 概要

plugin の store では、plugin ごとの台帳を「store の下の、plugin 名のフォルダ」に置き、全 plugin の本体を「store の下の `installed` フォルダ」にまとめて置いています。このため、名前がちょうど `installed` の plugin を入れると、その台帳のフォルダが本体置き場そのものになります。その plugin を削除すると、台帳のフォルダを丸ごと消す処理が本体置き場を消し、ほかの全 plugin の本体まで消えていました（issue #55）。

GitHub からインストールする場合は、リポジトリ名がそのまま plugin 名になります。そのため、第三者が `installed` というリポジトリで plugin を公開するだけで起こりえます。

修正に入る前に、issue へ噛み砕いた解説を書きました。また、upstream と jimoto-no-llm/mesh-llm に修正提案が無いことも確認しました。

修正は、issue で挙げた 2 案のうち小さいほう（案 A）にしました。`installed` という名前では plugin を入れられないようにし、すでにその名前で入っている plugin を消すときは、自分の台帳ファイルだけを消すようにしています。大文字と小文字を区別しないファイルシステムでは `Installed` も同じフォルダを指すので、大文字小文字を無視して比べます。

インストールの経路は 3 つ（ローカルのアーカイブ、GitHub、同梱の既定 plugin）あります。いずれも最後は同じ展開処理を通るので、そこで必ず拒否します。そのうえで、ローカルのアーカイブと GitHub については、もっと手前でも拒否します。GitHub の場合は、何もダウンロードしないうちに止まります。

新しく足したテストは 4 本です。修正を外すとすべて失敗し、修正を入れるとすべて通ることを確かめました。crate のテスト全体と clippy も通っています。

台帳の置き場所を本体置き場と重ならない配置に変える根本的な修正（案 B）は、既存の store の移し替えが要るので、今回は行っていません。

## 添付ファイル

- [plugin-manager のテスト結果](attachment/2026-11-02_011726_issue55_reserved_plugin_name_installed/plugin-manager-tests.log)
- [clippy の結果](attachment/2026-11-02_011726_issue55_reserved_plugin_name_installed/clippy.log)

## 核心発見サマリ

**結論**: `installed`（大文字小文字は無視）を plugin の予約名にした。ローカルのアーカイブと GitHub からのインストールでは、名前を決めた直後（GitHub ではダウンロード前）に拒否する。全経路が通る `extract_for_install` でも拒否する。予約名の plugin を `delete` するときは、共有ディレクトリを `remove_dir_all` しない。消すのは `plugin-install.json` と `default-off` だけにした。新しいテスト 4 本は、修正を外すと失敗し（ミューテーション確認）、修正を入れると `cargo test -p mesh-llm-plugin-manager` の 86 件がすべて通る。`cargo clippy --all-targets -D warnings` も通る。

## 前提・目的

- **背景**: issue #55。台帳 `<store root>/<名前>/plugin-install.json` と本体 `<store root>/installed/<名前>/` の置き場所が、`installed` という名前のときに重なる。`PluginStore::delete` は最後に `<store root>/<名前>` を `remove_dir_all` する
- **目的**: issue コメントで挙げた案 A（予約名として拒否し、既存の `installed` plugin は安全に消せるようにする）で塞ぐ
- **前提条件**: upstream（`8d7035a10`）と jimoto-no-llm/mesh-llm（`8959535ab`）には修正提案が無い（[issue コメント](https://github.com/miminashi/mesh-llm/issues/55#issuecomment-6560257308)）

## 環境情報

- ワークステーション（rustup 同梱の lld でリンク、`-j 8`）
- ブランチ: `fix/issue55-reserved-plugin-name`（`origin/main` `224b0d71f` から）
- 修正コミット: `738bbbc9c`

## 結果詳細

### 変更内容

| ファイル | 変更 |
|---|---|
| `mesh-llm-plugin-manager/src/store.rs` | `INSTALL_DIR_NAME`（`"installed"`）、`is_reserved_plugin_name`（`eq_ignore_ascii_case`）、`ensure_installable_plugin_name` を追加。予約名の `delete` は `plugin-install.json` と `default-off` だけを `remove_file` する |
| `mesh-llm-plugin-manager/src/install.rs` | `install_plugin_archive` の冒頭、`install_resolved_plugin` の冒頭（GitHub のリリース解決やダウンロードより前）、`extract_for_install` で `ensure_installable_plugin_name` を呼ぶ。`from_env` の `install_root` は `INSTALL_DIR_NAME` を使う |

- `is_valid_name` は GitHub のリポジトリ名の検査にも使われるので、変えていない
- 既存の `installed` plugin の読み込み・一覧・有効化は、これまでどおり動く。`plugins update installed` は拒否される（再インストールで同じ配置になるため）。いったん `delete` して、別の名前で入れ直す必要がある
- 同梱の既定 plugin は名前が決まっている（`installed` は無い）。それでも、`extract_for_install` で止まる

### 追加したテスト

| テスト | 確かめること |
|---|---|
| `store::tests::deleting_a_plugin_named_installed_keeps_the_other_plugins_files` | 予約名より前に入った `installed` を `delete` しても、`demo` の実行ファイルは残る。`installed/installed`・台帳・`default-off` は消え、`list()` は `["demo"]` になる |
| `store::tests::installed_is_reserved_in_any_case` | `installed` / `Installed` / `INSTALLED` は予約名。`installed-plugin` / `install` / `installed2` / `demo` は予約名ではない |
| `install::tests::a_local_archive_cannot_be_installed_under_a_reserved_name` | `install_plugin_archive` と `extract_for_install` が `installed` / `Installed` を拒否する。store にも install root にも何も作られない |
| `install::tests::a_github_repository_named_installed_is_refused_before_download` | `install_plugin("example/installed")` は、進捗イベントを 1 つも出さずに（リリース解決の前に）拒否される。ネットワーク不要 |

### ミューテーション確認

- `delete` の予約名の分岐だけを無効にすると、`deleting_a_plugin_named_installed_keeps_the_other_plugins_files` が失敗する（85 passed / 1 failed）
- `is_reserved_plugin_name` が常に false を返すようにすると、新しいテスト 4 本がすべて失敗する（82 passed / 4 failed）
- 元に戻すと 86 passed

## 残課題

- 案 B（台帳を `<store root>/metadata/<名前>/` などに移し、本体置き場と名前の空間を分ける）は未実施。将来ほかの予約名が要るようになったら検討する
- #54（台帳の `install_path` を信じて `remove_dir_all` する問題）は別 issue で未修正。#54 で `install_path` が「ちょうど `<store root>/installed/<名前>`」であることを確かめる際は、`INSTALL_DIR_NAME` を使える

## 参照レポート

- [plugin 名 `.` と `..` の拒否（#32）](./2026-11-01_223843_issue32_plugin_name_dot_fix.md)
- [台帳の name とディレクトリ名の照合（#33）](./2026-11-01_224440_issue33_plugin_metadata_name_check.md)
