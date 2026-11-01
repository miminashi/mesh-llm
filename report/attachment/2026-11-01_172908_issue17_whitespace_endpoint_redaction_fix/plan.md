# issue #17 の修正: 先頭に空白のある HTTP endpoint の credential を秘匿する

## Context

plugin の HTTP endpoint の address の先頭に空白があると、`url` crate（WHATWG URL parser）は空白を取り除いて普通の HTTP URL として扱う。そのため、health check も転送も通る。一方、秘匿するかどうかを決める `is_http_url`（`mesh/crates/mesh-llm-host-runtime/src/logging/policy.rs:178`）は「0 文字目が `http://` / `https://` か」で判定するので、HTTP の URL ではないと見なして伏せない。その結果、userinfo や `api_key` の query が `/api/status` に出てしまう。

調べたところ、同じ `is_http_url` を使う箇所がもう 1 つあった。`PluginEndpointSummary::redacted_for_network`（`plugin/types.rs:213`）で、ここも同じ理由で `/api/status` の plugin endpoint の `address` を伏せずに出す。今回の修正では、判定を 1 か所に集めて両方を直す。

parser が無視する文字は先頭の空白だけではない。WHATWG の仕様では、次の文字が無視される。

- 先頭と末尾の C0 制御文字と空白（U+0000〜U+0020）
- ASCII の tab・LF・CR（文字列の途中にあっても取り除く）

たとえば `ht\ttp://alice:s3cret@host` や、query のキーの途中に tab がある `?api\t_key=...` も、parser にとっては普通の URL・普通のキーになる。今の秘匿処理は、このような tab を見逃す（`redact_url_query` は `api\t_key` を秘密のキーとして扱わない）。

## 修正内容

### `mesh/crates/mesh-llm-host-runtime/src/logging/policy.rs`

1. `fn url_parser_view(value: &str) -> Cow<'_, str>` を追加する（非公開）
   - parser と同じように、先頭と末尾の C0 制御文字・空白を取り除き、途中の `\t` `\n` `\r` を消す
   - 取り除く文字が無ければ `Cow::Borrowed` を返す。普通の URL の出力は今と 1 バイトも変わらない
2. `pub(crate) fn redact_http_url(value: &str) -> Option<String>` を追加する
   - `url_parser_view` で正規化した文字列に、既存の `http_url_start(...) == Some(0)`（scheme の大文字・小文字を区別しない判定）を当てる
   - HTTP(S) の URL なら、正規化した文字列を既存の `redact_url_query` に通して `Some` で返す。そうでなければ `None` を返す
   - 今の `is_http_url` の doc comment（大文字の scheme の件）はこちらに移し、parser が無視する文字の話を書き足す
3. `is_http_url` を削除する。呼び出し元が 2 か所とも `redact_http_url` に置き換わり、使われなくなるため（clippy の dead_code 対策）。`http_url_start` と `redact_urls_in_text` は変えない

### 呼び出し元の 2 か所

- `network/metrics.rs:1142` の `TargetKey::redacted_label`: kind が `Endpoint` のときは `redact_http_url(&self.label).unwrap_or_else(|| self.label.clone())` を返す。local と remote の label は今までどおり書き換えない
- `plugin/types.rs:213` の `PluginEndpointSummary::redacted_for_network`: `if let Some(address) = ... && let Some(redacted) = redact_http_url(address)` の形にする。HTTP でない address（stdio、unix socket など）は今までどおりそのまま返す

metrics の集計に使うキー（`TargetKey` の生の label）は変えない。変えるのは、snapshot に出すときの表示だけ。

## テスト（追加）

- `logging/policy.rs` の tests
  - **parser と判定が一致すること**: `" http://..."`、`"\thttp://..."`、`"\u{1}https://..."`、`"http://... "`、`"ht\ttp://..."`、`"HTTP\n://..."`、`"127.0.0.1:9001"`、`"stdio:..."`、`" tcp://..."`、`""`、`"   "` のそれぞれについて、「`url::Url::parse` が成功し、scheme が http/https になる」ことと「`redact_http_url` が `Some` を返す」ことが一致する
  - 先頭の空白・tab・制御文字、`ht\ttp`、`?api\t_key=` のそれぞれで、`alice:s3cret` も `abc123` も出力に残らない
  - 普通の URL と大文字の scheme の URL の出力が、`redact_url_query` を直接呼んだ結果と完全に同じになる（回帰の確認）
- `network/metrics.rs` の tests
  - `leading_whitespace_endpoint_label_is_redacted_in_snapshot`: 先頭が空白の label で、`Success` のときも失敗（`Unavailable`）のときも、snapshot に userinfo と `api_key` が出ず、host は残る
- `plugin/types.rs` の tests
  - `leading_whitespace_address_is_redacted_for_network`: 先頭が空白の address の userinfo と query が伏せられる

修正を一時的に外して、追加したテストが失敗することも確かめる。

## 作業手順

1. worktree を作る: `git worktree add -b fix/issue17-whitespace-endpoint-redaction /home/ubuntu/projects/mesh-llm-worktrees/fix-issue17-whitespace-endpoint-redaction origin/main`（先に `git fetch origin`）
2. 上の修正とテストを実装する
3. 検証（ワークステーションには sccache が無いので `RUSTC_WRAPPER=` を付ける）
   - `cargo fmt -p mesh-llm-host-runtime` → `cargo fmt --all --check`
   - `cargo check -p mesh-llm-host-runtime`、`cargo clippy -p mesh-llm-host-runtime --all-targets -- -D warnings`
   - `cargo test -p mesh-llm-host-runtime --lib` で追加したテストを確かめ、PR 前に `cargo xtool repository affected-crates <変更したパス>` が出すクレートの `cargo test -p` をすべて実行する
   - mesh-llm バイナリから届くコードなので、`cargo check -p mesh-llm` と `cargo clippy -p mesh-llm --all-targets -- -D warnings` も実行する
4. コミットする: `fix(redaction): ...`。署名のトレーラーは付けない（memory: CI が Co-Authored-By: Claude 付きのコミットを拒否する）
5. レポートを書く: worktree の `report/<JST タイムスタンプ>_issue17_whitespace_endpoint_redaction_fix.md`。このプランを `report/attachment/<名前>/plan.md` にコピーし、`docs(report): ...` として別のコミットにする
6. push して PR を作る。本文は PR #30 と同じ形（概要 / 問題 / 修正内容 / 今回は直していない点 / テスト）にし、`Fixes #17` を入れる。`plugin/types.rs` の同じ穴も直したことを明記する

## 今回は扱わないもの

- percent-encoding したキー（`api%5Fkey=`）は、普通の URL でも今は伏せない。これは別の話なので、PR の「今回は直していない点」に書く
