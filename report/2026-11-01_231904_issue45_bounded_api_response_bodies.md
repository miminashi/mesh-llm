# Hub の API 応答とエラー応答を上限付きで読むように修正

- **実施日時**: 2026年11月1日 22:05 〜 23:19 JST (issue #45 の解説コメント投稿、upstream / jimoto-no-llm の再確認、修正とテスト)
- **報告日時**: 2026年11月1日 23:19 JST
- **作成者**: Claude Opus 5.5

## 概要

`skippy-hf-hub` は、Hugging Face Hub の API 応答とエラー応答の本体を、大きさを確かめずに最後までメモリにため込んでから読んでいました。接続先が信頼できないミラーやプロキシだと、終わりのない巨大な応答を送り続けられ、メモリが尽きてプロセスが落ちます。issue #45 で報告していたこの問題を直しました。

修正の前に、issue に噛み砕いた解説をコメントで書きました。あわせて、upstream と jimoto-no-llm/mesh-llm に同じ修正の提案が出ていないかを調べ直しましたが、どちらの main も該当箇所は直っておらず、関連する PR や issue もありませんでした。そのため、この fork で直すことにしました。

直し方は、PR #40（ファイルのダウンロードに上限を付けた修正）と同じ考え方です。受け取りながら量を数え、上限を超えたらその場で読むのをやめてエラーにします。この処理を共通の小さなモジュールにまとめ、`response.json()` / `response.text()` を呼んでいた全箇所をそこへ置き換えました。

上限は用途で変えています。普通の JSON 応答には余裕のある上限を、正当な応答でも大きくなり得る diff にはさらに大きい上限を付けました。エラー応答はメッセージが取れれば十分なので、先頭だけを残して残りは捨てます。エラー応答で上限を超えてもエラーにはせず、本来のエラー（ステータスコード）をそのまま返します。

正常な大きさの応答はこれまでどおり読めることを、既存のテストがすべて通ることで確かめました。新しく、上限ちょうどは読める、超えると止まる、申告サイズが大きすぎればすぐ止まる、巨大なエラー本体は先頭だけ残る、の各テストを足しています。

残りとしては、上限の値を利用者が変えられるようにはしていません。今のところ必要な場面がないので、要望が出たら考えます。

## 添付ファイル

- [cargo test の結果](attachment/2026-11-01_231904_issue45_bounded_api_response_bodies/cargo-test.log)

## 核心発見サマリ

**結論**: 新しい `src/body.rs` に上限付きで読む関数を 3 つ用意し、`skippy-hf-hub` の `response.json()` / `response.text()` 全 23 箇所を置き換えた。上限は、JSON 応答 64 MiB、diff 512 MiB、エラー本体 64 KiB（超えた分は捨てる）。超過時は `HFError::ResponseTooLarge` を返す（API 応答では `path` に URL が入る）。`cargo test -p skippy-hf-hub` は 256 件すべて成功し（新規 7 件を含む）、`cargo clippy --all-targets --all-features -- -D warnings` も警告なし。

## 前提・目的

- **背景**: issue #45（Low）。#18 / PR #40 の追加調査で見つけたもの。攻撃の前提は、`HF_ENDPOINT` に信頼できないミラーを指定している、または TLS を終端する中間者がいること
- **目的**: API 応答とエラー応答の本体を上限付きで読み、メモリを使い果たさせる DoS を防ぐ

## 環境情報

- ブランチ: `fix/issue45-bounded-api-bodies`（`origin/main` 35a64ce76 から作成）
- 修正コミット: dee9c66f5
- 確認した外部リポジトリ: `Mesh-LLM/mesh-llm` main e7e33fedf（2026-11-01）、`jimoto-no-llm/mesh-llm` main 8959535ab（2026-10-16）
- ワークステーション、lld でリンク、`-j 8`

## 結果詳細

### upstream / jimoto-no-llm の再確認

| リポジトリ | `src/error.rs:33` | 関連 PR / issue |
|---|---|---|
| Mesh-LLM/mesh-llm | `response.text()` のまま | なし（#956 は OpenAI 応答側の話で無関係） |
| jimoto-no-llm/mesh-llm | `response.text()` のまま | なし |

検索語: `skippy-hf-hub`, `hf-hub`, `response body limit`, `unbounded response`, `error body size`, `HttpErrorContext` など。

### 追加したヘルパー（`skippy/crates/skippy-hf-hub/src/body.rs`）

| 関数 | 用途 | 上限超過時 |
|---|---|---|
| `read_bytes_limited(response, limit)` | 本体を上限付きで集める。`Content-Length` が上限を超えていれば読む前に失敗する。先に確保する容量は 1 MiB まで | `ResponseTooLarge` |
| `json_limited(response)` | 64 MiB まで読んで `serde_json::from_slice` | `ResponseTooLarge`（JSON が壊れていれば `HFError::Json`） |
| `text_limited(response, limit)` | diff 用。不正な UTF-8 は置換する | `ResponseTooLarge` |
| `text_prefix(response, limit)` | エラー本体用。先頭 64 KiB だけを残し、失敗しない | 切り詰め |

### 置き換えた箇所

- `error.rs` `HttpErrorContext::from_response` → `text_prefix`
- `repository/commits.rs` の `get_commit_diff` / `get_raw_diff` → `text_limited`（512 MiB）
- それ以外の `response.json()` はすべて `json_limited` へ: `pagination.rs`、`xet.rs`、`repository/{mod,listing,commits,upload}.rs`、`buckets/mod.rs`、`spaces.rs`、`users.rs`

### 挙動の変化

- JSON のデコードに失敗したときのエラーが、`HFError::Request`（reqwest の decode error）から `HFError::Json` に変わる。リポジトリ内に、この違いに依存しているコードはない
- `text()` は `Content-Type` の charset を見てデコードしていたが、置き換え後は UTF-8 として扱う（Hub の応答は UTF-8）

### 追加したテスト

- `body::tests`: 上限ちょうどは読める／chunked で上限を超えると `ResponseTooLarge`／`Content-Length` が上限超過なら読む前に失敗／正常な JSON を読める／壊れた JSON は `Json`／`text_prefix` が切り詰める
- `error::tests::from_response_keeps_only_a_prefix_of_a_huge_error_body`: 64 MiB の chunked な 500 応答で、`body` が 64 KiB に収まり、ヘッダの `X-Error-Message` は取れる

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-issue45-bounded-api-bodies
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -j 8 -p skippy-hf-hub
cargo clippy -j 8 -p skippy-hf-hub --all-targets --all-features -- -D warnings
```

## 参照レポート

- [issue #19 HTTP 再開のコミット固定](./2026-11-01_212352_issue19_http_resume_commit_pin_fix.md)

## 残課題

- 上限は crate 内の定数で、呼び出し側からは変えられない。巨大な diff を扱う必要が出たら、builder から上限を指定できるようにする
- wasm ターゲットではビルドしていない（ワークステーションにターゲットが無く、CI でも使っていない）。`bytes_stream` は download.rs ですでに使っているので、同じ条件で動くはず
