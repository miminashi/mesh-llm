# 先頭に空白のある endpoint URL の秘密が漏れる問題の修正 — 判定を URL parser に揃える

- **実施日時**: 2026年11月1日 00:27 〜 18:05 JST (issue #17 の修正、テスト追加、フルテスト。途中、メモリ不足でテストが止められ、lld に切り替えて再実行するまで待機した)
- **報告日時**: 2026年11月1日 18:05 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm では、plugin が推論用の HTTP endpoint を登録できます。その URL には、ユーザー名とパスワードや API キーを埋め込むことがあります。mesh-llm は管理 API（`/api/status`）でこの URL を見せる前に、秘密の部分を伏せています。issue #17 は、URL の先頭に空白が 1 文字入っているだけで、この伏せる処理が働かなくなる、という問題でした。

原因は、URL を「HTTP の URL か」と判定する規則が場所ごとに違っていたことです。通信に使う URL parser は、先頭の空白などを取り除いて普通の URL として扱います。そのため、health check も転送も通ります。一方、伏せる処理は「文字列の先頭が `http://` か」だけで判定していたので、空白付きの URL を HTTP の URL ではないと見なし、そのまま出していました。

修正では、伏せる処理の判定を URL parser に揃えました。parser が無視する文字（両端の空白・制御文字と、文字列のどこにあっても取り除かれる tab・改行）を先に取り除いてから判定し、伏せます。普通の URL の出力は今までとまったく同じです。

調べる中で、同じ判定を使う箇所がもう 1 つ見つかりました。`/api/status` に出る plugin endpoint の一覧の address です。issue に書かれていた metrics の label と同じ穴があったので、両方を 1 つの関数にまとめて直しました。

確認のために、URL parser の判断と秘匿の判定が一致することを、17 通りの入力で確かめるテストを書きました。修正を外すと、追加したテストのうち 4 個が失敗します。フルテストでは 3 件失敗しましたが、3 件とも今回の変更とは関係のない、時間切れによる失敗でした。2 件は単独で再実行すると通り、残る 1 件は main のコードでも同じように失敗しました。

作業の途中で、このワークステーションでは GNU ld でのリンクがメモリを使いすぎ、テストがメモリ不足で止められました。別のセッションから、rustup に同梱されている lld を使う方法が共有され、それに切り替えて再実行しました。このレポートは修正と同じ PR に含めています。

## 添付ファイル

- [実装プラン](attachment/2026-11-01_172908_issue17_whitespace_endpoint_redaction_fix/plan.md)
- [フルテストの抜粋（結果行・失敗・追加テスト）](attachment/2026-11-01_172908_issue17_whitespace_endpoint_redaction_fix/full-test-excerpt.log)

## 核心発見サマリ

**結論**: `logging/policy.rs` の `is_http_url` を `redact_http_url(value) -> Option<String>` に置き換えた。WHATWG URL parser が無視する文字（先頭と末尾の U+0000〜U+0020、途中の `\t` `\n` `\r`）を `url_parser_view` で取り除いてから、大文字・小文字を区別しない scheme の判定（`http_url_start(..) == Some(0)`）を行い、正規化した文字列を `redact_url_query` に通す。呼び出し元は `TargetKey::redacted_label`（routing metrics）と `PluginEndpointSummary::redacted_for_network`（plugin endpoint 一覧）の 2 か所。追加したテストは 5 個で、修正を外すと 4 個が失敗する（残る 1 個は回帰テスト）。`cargo test -j 8 -p mesh-llm-host-runtime` の結果は 4314 passed / 3 failed / 26 ignored、`-p skippy-hf-hub` はすべて通った。失敗した 3 件はどれも時間切れ（`MaintenanceCancelled` / `Elapsed`）で、2 件は単独で再実行すると通り、gossip の QUIC テストは `origin/main`（`1a0f96438`）のコードでも同じように失敗した。fmt と clippy（`mesh-llm-host-runtime`・`mesh-llm`、`-D warnings`）は問題なし。

## 前提・目的

- **背景**: issue #17（元の報告は jimoto-no-llm/mesh-llm#175）。`url::Url::parse` は `" https://alice:s3cret@host/v1?api_key=..."` を受け付けるが、`is_http_url` は 0 文字目が scheme でないと偽を返し、`redacted_label` が元の文字列を返していた
- **目的**: 秘匿の判定を、実際に URL を解釈する parser と揃える。普通の URL・大文字の scheme の秘匿と、local・remote の label の素通しは変えない
- upstream（Mesh-LLM/mesh-llm）と jimoto-no-llm/mesh-llm には、2026-11-01 時点で修正の提案が無いことを確認済み（issue #17 のコメント参照）

## 環境情報

- ワークステーション（RAM 31GB、aws-v100 は使っていない）
- worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue17-whitespace-endpoint-redaction`（ブランチ `fix/issue17-whitespace-endpoint-redaction`、base `6e83e2d0a`）
- 修正コミット: `9a050cec0`
- 変更したファイル: `mesh/crates/mesh-llm-host-runtime/src/logging/policy.rs`、`network/metrics.rs`、`plugin/types.rs`

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-issue17-whitespace-endpoint-redaction
# mold / lld がシステムに無いので rustup 同梱の lld を使う（GNU ld だとリンク 1 本で約 5GB 使う）
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=$PWD/target
cargo test -j 8 -p mesh-llm-host-runtime --lib -- http_url_redaction leading_whitespace
cargo fmt --all --check
cargo clippy -p mesh-llm-host-runtime --all-targets -- -D warnings
cargo clippy -p mesh-llm --all-targets -- -D warnings
cargo xtool repository affected-crates $(git diff --name-only origin/main)   # test_crates: mesh-llm-host-runtime, skippy-hf-hub
cargo test -j 8 -p mesh-llm-host-runtime
cargo test -j 8 -p skippy-hf-hub
```

## 結果詳細

### 追加したテスト

| テスト | 内容 | 修正なし | 修正あり |
|---|---|---|---|
| `policy::http_url_redaction_tests::redaction_gate_agrees_with_url_parser` | 17 通りの入力で、`Url::parse` が http/https として成功することと、`redact_http_url` が `Some` を返すことが一致する | FAILED | ok |
| `policy::http_url_redaction_tests::credentials_hidden_by_parser_ignored_characters_are_redacted` | 先頭の空白・tab・NUL、末尾の空白、`ht\ttp`、`api\t_key` で、userinfo も `api_key` の値も残らない | FAILED | ok |
| `policy::http_url_redaction_tests::plain_http_urls_are_redacted_exactly_as_before` | 普通の URL と大文字の scheme の URL の出力が、`redact_url_query` を直接呼んだ結果と完全に同じ | ok | ok |
| `metrics::tests::leading_whitespace_endpoint_label_is_redacted_in_snapshot` | `Success` と `Unavailable` の両方で、snapshot の label が伏せられ、host は残る | FAILED | ok |
| `plugin::types::tests::leading_whitespace_address_is_redacted_for_network` | plugin endpoint の address が伏せられる | FAILED | ok |

「修正なし」は、`redact_http_url` の正規化だけを一時的に外して（`Cow::Borrowed(value)` に差し替えて）実行した結果。

### フルテストで失敗した 3 件

| テスト | 失敗の内容 | 単独で再実行 | main のコード |
|---|---|---|---|
| `api::routes::logs::delete::tests::accepted_delete_timeout_returns_pending_without_a_failure_audit` | `MaintenanceCancelled` | ok | 未確認 |
| `sdk::embedded_lifecycle_tests::cancelling_startup_releases_runtime_and_restarts` | `startup is dialing the fixture: Elapsed(())` | ok | 未確認 |
| `mesh::gossip::tests::signed_records::bounded_exchange::oversized_snapshots_complete_real_quic_exchanges_in_both_directions` | `Elapsed(())`（実際の QUIC 通信） | FAILED | FAILED |

どれも、変更したファイル（ログの秘匿、metrics、plugin の型）には触れないテストで、時間切れによる失敗。

## 副次発見

- `PluginEndpointSummary::redacted_for_network`（`/api/status` の plugin endpoint 一覧）にも、同じ判定による漏れがあった。issue には書かれていなかったが、この PR で一緒に直した
- このワークステーションでは mold も lld も入っておらず、`scripts/cargo-linker` が GNU ld に fallback する。テストバイナリのリンクがメモリを使いすぎ、最初のフルテストはメモリ不足で Claude Code に止められた。rustup に同梱されている `gcc-ld` を PATH の先頭に足すと、`scripts/cargo-linker-linux-x86_64 --mesh-probe` が `lld` を返し、メモリ不足にならずに完走した（別のセッションからの共有による）

## 残課題

- percent-encoding したキー（`api%5Fkey=`）は、普通の URL でも今は伏せていない。今回の空白の件とは別の話なので、扱っていない
- gossip の QUIC テストが、このワークステーションでは main のコードでも時間切れになる。CI で通るかを PR で確かめる
- PR マージ後に worktree を片付ける
