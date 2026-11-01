# plugin 名の . と .. を拒否する修正 — plugins delete .. で鍵が消える問題を解消

- **実施日時**: 2026年11月01日 21:30 〜 22:40 JST (issue #32 の解説コメント、upstream と jimoto-no-llm での修正状況の確認、修正とテスト)
- **報告日時**: 2026年11月01日 22:40 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm の plugin は、ホームディレクトリの下にある plugin store というディレクトリに、plugin 名と同じ名前のディレクトリを作って管理されている。名前の検査は「使っている文字が英数字と一部の記号だけか」しか見ていなかったため、「今のディレクトリ」と「1 つ上のディレクトリ」を表す特別な名前（`.` と `..`）も通っていた。そのため、`..` という名前の plugin を消すように指示すると、plugin store の親ディレクトリの中身、つまりノードの鍵まで消えてしまう、というのが issue #32 である。

まず issue に、仕組みと危険の大きさを噛み砕いた解説コメントを書いた。この問題は手元でコマンドを打ったときにしか起きず、ネットワーク越しには起こせない。それでも、鍵が消えるとノードが別人として扱われ、バックアップが無ければ元に戻せない。

次に、upstream と jimoto-no-llm の fork で、同じ問題への修正提案が出ていないかを調べた。どちらの最新のコードも同じ検査のままで、関係する issue・PR・ブランチは見つからなかった。そのため、この fork で独自に直すことにした。

修正は、名前の検査で `.` と `..` を拒否するという小さなもの。この検査は GitHub のリポジトリ名の確認にも使われているが、GitHub ではもともとそういう名前のリポジトリを作れないので、そちらへの影響は無い。あわせて、ローカルのアーカイブから plugin を入れるときのエラー文が、実際の検査とは違う規則を説明していたので、検査の内容に合わせて書き直した。

テストは 2 つ足した。1 つは、名前の検査とインストール時の参照の解析が `.` と `..` を拒否することを確かめる。もう 1 つは、plugin store の削除・保存・読み込み・既定の無効化の記録が、どれも `.` と `..` を受け付けず、親ディレクトリに置いた鍵のファイルや store 自体に触れないことを確かめる。どちらのテストも、修正を外すと失敗し、修正後は通ることを確認した。

CLI のコマンドを処理する層（`plugins delete` の入口）にもテストを足そうとしたが、その crate は llama.cpp の静的ビルドが必要で、このワークステーションではビルドできなかった。確かめられないテストは入れないことにし、削除処理の本体である store 側のテストで代えた。

## 添付ファイル

- [修正前のテスト結果（新しいテスト 2 件が失敗）](attachment/2026-11-01_223843_issue32_plugin_name_dot_fix/test-before-fix.txt)
- [修正後のテスト結果（79 件すべて pass）](attachment/2026-11-01_223843_issue32_plugin_name_dot_fix/test-after-fix.txt)

plan mode は使っていないため、プランファイルは無い。

## 核心発見サマリ

**結論**: `is_valid_name`（`mesh/crates/mesh-llm-plugin-manager/src/source_ref.rs`）に `value != "." && value != ".."` を足した。store の `save` / `try_load` / `load` / `delete` / `set_default_turned_off` / `default_turned_off` は、どれもこの関数で名前を検査してから `plugin_dir(name)`（`root.join(name)`）を使うので、1 か所の修正で全部が `.` と `..` を拒否するようになる。追加したテスト `source_ref::tests::dot_names_are_not_plugin_names` と `store::tests::dot_names_never_reach_the_store_root_or_its_parent` は、修正前は 2 件とも FAILED、修正後は `mesh-llm-plugin-manager` のテスト 79 件がすべて pass した。clippy（`--all-targets -D warnings`）と `cargo fmt --check` も通った。upstream（`e7e33fedf`、2026-11-01）と jimoto-no-llm（`8959535ab`、2026-10-16）は、どちらも修正前と同じコードで、修正提案は無い。

## 前提・目的

- **背景**: issue #32。#14（PR #30）を直したときに見つかった。`is_valid_name` が `.` と `..` を通すため、`mesh-llm plugins delete ..` で `~/.mesh-llm` の中身（ノードの鍵 `key` や `mesh-id`）が消える。`plugins delete .` では plugin store が丸ごと消える
- **目的**: `.` と `..` を plugin 名として拒否し、store の操作が store root やその親に届かないようにする。ついでに、`install_plugin_archive` のエラー文と実際の検査の食い違いを直す

## 環境情報

- ワークステーション（32 コア、メモリ 31GB）、rustup 同梱の lld でリンク、`RUSTC_WRAPPER=`、`CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target`
- ブランチ: `fix/issue32-plugin-name-dot`（`origin/main` の `591e7d015` から作成）
- 修正コミット: `bdfe45821`

## 結果詳細

### 変更内容

| ファイル | 変更 |
|---|---|
| `mesh/crates/mesh-llm-plugin-manager/src/source_ref.rs` | `is_valid_name` で `.` と `..` を拒否。`parse_catalog_ref` のエラー文に「`.` と `..` は不可」を追記。テスト `dot_names_are_not_plugin_names` を追加 |
| `mesh/crates/mesh-llm-plugin-manager/src/install.rs` | `install_plugin_archive` のエラー文を、実際の検査（英数字と `.` `_` `-`、ただし `.` と `..` は不可）に合わせた |
| `mesh/crates/mesh-llm-plugin-manager/src/store.rs` | テスト `dot_names_never_reach_the_store_root_or_its_parent` を追加 |

エラー文は、検査を厳しくする（小文字・数字・`-` だけにする）のではなく、エラー文のほうを検査に合わせた。検査を厳しくすると、大文字や `.` `_` を含む名前で入れてある既存の plugin を、`delete` などで扱えなくなるおそれがあるため。

### テストの内容

- `dot_names_are_not_plugin_names`: `is_valid_name` が `.` と `..` を拒否し、`parse_install_ref` が `.`・`..@v1.0.0`・`mesh-llm/..` などをエラーにすることを確かめる。`...`・`.hidden`・`a..b` などの、ドットを含むだけの名前は今までどおり通ることも確かめる
- `dot_names_never_reach_the_store_root_or_its_parent`: store root を `<tmp>/dot-mesh-llm/plugins` にし、親に `key` と `plugin.json`、root に `plugin.json` を置く。そのうえで `.` と `..` について、`delete`・`try_load`・`load`・`save`・`set_default_turned_off(true/false)` がエラーを返し、`default_turned_off` が false を返すことを確かめる。最後に、置いたファイルの中身が変わっておらず、`default-off` も作られておらず、既存の plugin（blackboard）が読めることを確かめる

### 実行結果

- 修正を外した状態: 新しいテスト 2 件が FAILED（`test-before-fix.txt`）
- 修正後: `cargo test -p mesh-llm-plugin-manager` で 79 件 pass（`test-after-fix.txt`）
- `cargo clippy -p mesh-llm-plugin-manager --all-targets -- -D warnings`: 警告なし
- `cargo fmt -p mesh-llm-plugin-manager --check`: 差分なし

### upstream / jimoto-no-llm の確認（2026-11-01 時点）

- upstream（`Mesh-LLM/mesh-llm`、`e7e33fedf`）: `is_valid_name` と `plugin_dir` は修正前と同じ。2026-10 以降に `source_ref.rs` / `store.rs` / `install.rs` を変えたコミット（#2337・#2226・#2614・#2712）は名前の検査に触れていない。これらのファイルか `mesh-llm-commands/src/plugin.rs` を変える open の PR は #2654 だけで、その差分にも検査や削除の変更は無い
- jimoto-no-llm/mesh-llm（`8959535ab`）: 同じコード。関係する issue・PR・ブランチは無い
- 詳細は issue #32 のコメントに書いた

## 副次発見

- `mesh-llm-commands` は llama.cpp の静的 ABI（`skippy-ffi`）が必要で、`.deps` が無い worktree では build script（`build-llama.sh`）が exit 127 で失敗する。fork CI でもテスト対象外のクレート。そのため、`delete_in`（CLI の入口）を通すテストは足していない
- テストの実行中、別セッション（issue #33 の worktree）が同じ target ディレクトリで `mesh-llm-host-runtime` のテストをビルドしていて、cargo のビルドロック待ちで 10 分以上止まった。並行して重い cargo を走らせないよう、こちらは追加で起動せずに待った

## 残課題

- `plugins delete ..` を実際の CLI から打って確かめることはしていない（`mesh-llm-commands` がこの環境でビルドできないため）。必要なら、llama.cpp をビルドした環境（aws-v100 など）で、一時的な `MESH_LLM_PLUGIN_DIR` を使って確かめる
