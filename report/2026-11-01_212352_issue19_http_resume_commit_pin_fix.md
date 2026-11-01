# HF cache 取得で新旧のファイルが混ざる問題の修正

- **実施日時**: 2026年11月1日 21:18 〜 21:25 JST (issue #19 の修正: cache 取得の GET を HEAD で確定した commit に固定し、回帰テストを追加)
- **報告日時**: 2026年11月1日 21:25 JST
- **作成者**: Claude Opus 5.5

## 概要

Hugging Face からモデルを手元の cache に落とす処理で、新旧のファイルが混ざったものを、古い版の完成品として保存してしまう問題（issue #19）を直しました。

この処理は、まず HEAD でファイルの ID（ETag）とブランチが指す commit を聞き、続く GET で中身を受け取ります。cache 内のファイル名も、書きかけのファイル名も、HEAD で聞いた ID と commit で決まります。ところが GET は、HEAD と同じく「`main` の最新」という、後から変わりうる宛先に送っていました。HEAD と GET の間に `main` が更新されると、新しい版の中身が届きます。それでも、古い版の名前のまま保存されていました。

直し方は単純で、GET の宛先を、HEAD で確定した commit を指す URL に変えました。commit を指す URL の中身は変わらないので、途中から再開する場合も最初から取る場合も、名前と中身が必ず一致します。HEAD は今までどおり `main` に送ります。ブランチを最新の commit に解決するのと、取得済みかどうかの判定（`If-None-Match`）に必要だからです。

issue では、`If-Range` を付ける直し方も挙げていました。ただ、HF の GET は配信用の CDN にリダイレクトされ、CDN の ETag は HF の ETag とは別物です。`If-Range` を付けると常に「別物」と判定されて、途中からの再開が効かなくなります。このため採りませんでした。

テストでは、HEAD と GET の間にブランチが動く試験サーバを用意し、途中から再開する場合と最初から取る場合の 2 件を足しました。修正前のコードでは、どちらも新旧が混ざった中身（または新しい版の中身）が保存されて失敗することを確かめました。修正後は、既存のテストも含めてすべて通ります。本物の HF でも、commit を指す URL で途中からの取得（206）ができることを確認しました。

同じ方針の修正は、外部の人の PR #25 にも出ています。今回はユーザーの判断で独自に PR を作り、#25 には触れていません。最初から取る場合のテストと、取り込み元からの変更記録（`SOURCE_PROVENANCE.json`）の更新は、#25 にはない部分です。

残課題として、cache を使わずに指定のディレクトリへ直接落とす経路も、同じく途中からの再開で混ざる可能性があります。ただ、この経路は mesh-llm からは呼ばれていません。直すには commit の取り方から設計し直す必要があるため、今回は対象外にしました。

## 添付ファイル

- [実装プラン](attachment/2026-11-01_212352_issue19_http_resume_commit_pin_fix/plan.md)
- [skippy-hf-hub のテスト結果（修正後）](attachment/2026-11-01_212352_issue19_http_resume_commit_pin_fix/skippy-hf-hub-test.log)
- [新しいテストを修正前のコードで動かした結果](attachment/2026-11-01_212352_issue19_http_resume_commit_pin_fix/branch-moves-test-without-fix.log)

## 核心発見サマリ

**結論**: GET の宛先を、HEAD が返した `X-Repo-Commit` の `/resolve/<commit>/<file>` に固定しました。修正前は、partial ファイル `abcd` から再開すると `abcdUVWXYZ` が、partial ファイルなしで取ると新しい版の `ABCDUVWXYZ` が、古い ETag・commit の名前で確定していました。修正後は、どちらも HEAD の版 `abcdefghij` になります。`cargo test -p skippy-hf-hub` は lib 226、統合 2、doc 21 がすべて通り、fmt と clippy（`--all-targets -D warnings`）も通りました。

## 前提・目的

- **背景**: issue #19（Medium）。jimoto-no-llm/mesh-llm#358 でも同じ問題が報告され、手元の試験で `abcdUVWXYZ` が再現されている。upstream（Mesh-LLM/mesh-llm）と jimoto-no-llm に修正提案はない（2026-11-01 に確認済み。issue #19 のコメントを参照）
- **目的**: cache に確定するファイルの中身を、そのファイルの名前（HEAD の ETag と commit）と必ず一致させる。通常の再開と、不正な `Content-Range` からの取り直しは今までどおり動かす

## 環境情報

- ブランチ: `fix/issue19-http-resume-identity`（`origin/main` の `5edcc83f7` から）
- 変更ファイル:
  - `skippy/crates/skippy-hf-hub/src/repository/download.rs`
  - `skippy/crates/skippy-hf-hub/SOURCE_PROVENANCE.json`（`imported_sha256` と `local_deltas`）
- ビルド: ワークステーションで rustup 同梱の lld を使用（CLAUDE.md のメモリ対策）

## 結果詳細

### コードの変更

`download_file_to_cache_network` で、`admit_commit` を通した `commit_hash` から、既存の `HFClient::download_url` を使って GET 用の URL を作り、`http_download_response` に渡すようにした。`revision` がもともと commit のときは、URL は今までと同じになる。

### 採らなかった案

| 案 | 採らなかった理由 |
|---|---|
| `If-Range: "<ETag>"` を付ける | GET は LFS / xet の CDN にリダイレクトされ、CDN の ETag は HF の ETag と一致しない。常に 200（全体）が返り、再開が効かなくなる |
| GET 応答の ETag・commit を HEAD と照合する | リダイレクト先の応答に `X-Repo-Commit` が付かないので照合できない |

### テスト

`download.rs` の `tests` module に追加した（試験サーバ `start_moving_branch_server`）。HEAD には古い commit と古い ETag を返す。`/resolve/main/` への GET には同じサイズの新しい版 `ABCDUVWXYZ` を、`/resolve/<古い commit>/` への GET には古い版 `abcdefghij` を返す。

| テスト | 修正前 | 修正後 |
|---|---|---|
| `cache_resume_after_branch_moves_keeps_the_head_commit_bytes`（partial `abcd` から再開） | `abcdUVWXYZ` で失敗 | 通る |
| `cache_download_after_branch_moves_keeps_the_head_commit_bytes`（partial なし） | `ABCDUVWXYZ` で失敗 | 通る |

既存の `cache_download_resumes_http_incomplete_blob`、`cache_download_restarts_when_partial_response_is_invalid`、`local_dir_download_resumes_http_partial_file` と、`content_range_matches` のテストは変えずに通った。

### 本物の HF での確認

`openai-community/gpt2` で、HEAD で得た commit を指す `/resolve/<commit>/model.safetensors` と `/resolve/<commit>/config.json` に、Range 付きでリダイレクトを追って GET し、どちらも 206 が返ることを確かめた。

## 再現方法

```bash
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -j 8 --locked -p skippy-hf-hub
cargo fmt -p skippy-hf-hub -- --check
cargo clippy -j 8 --locked -p skippy-hf-hub --all-targets -- -D warnings
```

修正前の失敗を見るには、`download_file_to_cache_network` の `http_download_response` に渡す `&commit_url` を `&url` に戻して `cargo test -p skippy-hf-hub --lib branch_moves` を実行する。

## 残課題

- `download_file_to_local_dir`（`local_dir` を指定した取得）も、HEAD と GET を `revision` に送り、partial ファイルに ID を持たないため、再開すると混ざる可能性がある。mesh-llm からは呼ばれていない。HEAD がリダイレクトを追うので commit が取れず、直すには別の設計が要る
- PR #25（外部の人の同じ方針の修正）の扱いは、ユーザーが決める

## 参照レポート

- [issue #18 の修正（同じ download.rs の bytes 取得の上限）](./2026-11-01_175353_issue18_hf_hub_bytes_download_limit_fix.md)
