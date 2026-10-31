# bucket sync が保存先の外に書き込む問題の修正 — ファイルに触る前に path を検査

- **実施日時**: 2026年10月31日 23:50 〜 2026年11月1日 01:00 JST (issue #13 の解説コメント、upstream・jimoto-no-llm の修正提案の確認、修正とテスト追加、PR 作成)
- **報告日時**: 2026年11月1日 01:00 JST
- **作成者**: Claude Opus 5.5

## 概要

`skippy-hf-hub` は Hugging Face Hub のクライアントライブラリで、upstream がソースごとこの repo に取り込んで持っています。その中に、Hub 上の bucket とローカルのディレクトリを同期する機能（bucket sync）があります。issue #13 は、ダウンロード方向の sync が、サーバの返したファイル名を検査せずに保存先の path へつないでいるため、`..` や絶対 path を含む名前で保存先の外にファイルを作ったり消したりできてしまう、という問題でした。保存先の中にある既存の symlink を経由して外に出ることも防いでいませんでした。

まず issue に噛み砕いた解説を書きました。いまの mesh-llm には、ダウンロード方向の sync を呼んでいる箇所がありません。CI や普段の動作で突かれる経路はなく、「ライブラリの公開 API に穴があるが、誰も呼んでいない」状態です。それでも、upstream をマージしたときにいつのまにか使われ始めるおそれがあることと、修正が小さいことから、この fork で直すことにしました。upstream と jimoto-no-llm/mesh-llm を確認しましたが、どちらにも修正提案はまだありませんでした。

修正では、サーバ由来の path を保存先の下の安全な位置に変換する関数を 1 つ足しました。path は「普通の名前」だけからできていなければならず、`..`・先頭の `.`・絶対 path・空文字列は拒否します。保存先の中にすでにある部分が symlink の場合も拒否します。ダウンロードが一時的に書き込む `.incomplete` ファイルも、同じように確かめます。

この検査は、ダウンロードと削除のどちらについても、ファイルに触る前にまとめて行います。不正な名前が 1 つでもあれば、何も作らず何も消さずにエラーにします。途中まで同期した中途半端な状態は残りません。

修正を外すと新しいテストのうち 4 件が失敗し、修正を入れると通ることを確かめました。取り込み元からの変更点を記録している `SOURCE_PROVENANCE.json` にも、今回の変更を追記しました。

このレポートは修正と同じ PR に含めています。PR マージ後に worktree を片付けます。

## 添付ファイル

なし（plan mode を使っていないため、プランファイルはない）

## 核心発見サマリ

**結論**: `skippy/crates/skippy-hf-hub/src/buckets/sync.rs` に `contained_local_path(root, rel)` を追加した。`Path::new(rel).components()` がすべて `Component::Normal` であることを要求し（`ParentDir`・`CurDir`・`RootDir`・`Prefix`・空文字列を拒否）、各 component を push するたびに `symlink_metadata` で symlink でないことを確かめ、最後に `<path>.incomplete` も symlink でないことを確かめる。`execute_download_plan` は冒頭で Download / Delete の全操作についてこれを呼んでから、`create_dir_all`・xet ダウンロード・`remove_file`・空ディレクトリ掃除に検査済みの path だけを使う。テストは新規 6 件を含めて `buckets::sync` 36 passed。検査を一時的に無効にすると、新規 6 件のうち 4 件（拒否系）が FAILED になる。

## 前提・目的

- **背景**: issue #13（元は jimoto-no-llm/mesh-llm#359）。bucket sync のダウンロードで、listing の key を `params.local_path.join(&op.path)` にそのまま使っていた
- **目的**: サーバ由来の path によるファイル操作を、保存先の中に閉じ込める。正当な nested file の同期と削除は今までどおり動かす
- **前提条件**: ダウンロード方向の sync を呼ぶ箇所は、いまの mesh-llm・CI・scripts にない（issue の解説コメント参照）

## 環境情報

- ワークステーション（aws-v100 は使っていない）
- worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-bucket-sync-path-containment`（ブランチ `fix/bucket-sync-path-containment`、base `6e83e2d0a`）
- 変更ファイル: `skippy/crates/skippy-hf-hub/src/buckets/sync.rs`、`skippy/crates/skippy-hf-hub/SOURCE_PROVENANCE.json`

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-bucket-sync-path-containment
export RUSTC_WRAPPER= CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -p skippy-hf-hub --lib buckets::sync
cargo test -p skippy-hf-hub --all-features
cargo clippy -p skippy-hf-hub --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

## 結果詳細

| 確認 | 結果 |
|---|---|
| `cargo test -p skippy-hf-hub --lib buckets::sync` | 36 passed（新規 6 件を含む） |
| 検査を一時的に無効にしたテスト | 拒否系 4 件が FAILED（`rejects_escaping_paths` / `rejects_existing_symlinks` / `rejects_escaping_download_before_writing` / `rejects_escaping_delete_before_removing_anything`） |
| `cargo test -p skippy-hf-hub --all-features` | lib 225 passed、`tests/future_size.rs` 2 passed、doc 21 passed |
| `cargo clippy -p skippy-hf-hub --all-targets --all-features -- -D warnings` | 警告なし |
| `cargo fmt --all --check` | pass |
| `cargo xtool repository affected-crates` | `test_crates` は `skippy-hf-hub` のみ |

追加したテスト:

| テスト | 確かめること |
|---|---|
| `test_contained_local_path_accepts_nested_relative_paths` | `a.txt`・既存の `models/existing.bin`・まだない `models/new/deep/file.gguf` は保存先の下の path になる |
| `test_contained_local_path_rejects_escaping_paths` | `""`・`..`・`../outside.txt`・`a/../../outside.txt`・`a/..`・`./a.txt`・`/etc/passwd` を拒否 |
| `test_contained_local_path_rejects_existing_symlinks`（unix） | 外を指すディレクトリ symlink の下・symlink そのもの・`.incomplete` が symlink のファイルを拒否 |
| `test_download_plan_rejects_escaping_download_before_writing` | `../escaped/payload.txt` のダウンロードはエラーになり、外にディレクトリが作られない |
| `test_download_plan_rejects_escaping_delete_before_removing_anything` | 正当な削除と `../victim.txt` の削除が混ざった plan はエラーになり、どちらのファイルも消えない |
| `test_download_plan_still_deletes_contained_nested_files` | 正当な nested file は今までどおり削除され、空になった親ディレクトリも片付く |

## 副次発見

- `Path::components()` は途中の `.` を捨てる（`a/./b` は `a`、`b` になる）。これは保存先の外に出ないので、受け入れている。拒否するのは先頭の `.`（`CurDir` として出てくる）だけ
- ローカル側の一覧（`list_local_files`）は `DirEntry::file_type()` で symlink を辿らないので、削除対象に symlink やその先のファイルが入ることはもともとない
- `SOURCE_PROVENANCE.json` の hash を自動で照合する仕組みは repo に見当たらない（記録のみ）。今回は `local_deltas` に追記し、`imported_sha256` の `src/buckets/sync.rs` を更新した

## 残課題

- 検査と書き込みの間に、ローカルの別プロセスが symlink を差し込む競合（TOCTOU）は防いでいない。保存先に書き込めるローカルの攻撃者が前提になるので、今回の対象（悪意のあるサーバ）からは外した
- upstream が同じ箇所を直したら、マージのときに upstream 側の修正を採用する
