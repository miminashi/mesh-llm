# 修正状況 全件一覧

対象は、前回のトリアージで「該当」とした 42 件（fork `jimoto-no-llm/mesh-llm` の issue）。手元の `main` `95f9b4810`（2026-10-22）のコードを読んで判定した。判定はコードを読んだだけで、ビルドや再現テストはしていない。

判定の区分:
- **修正済み**: 指摘された処理が、今のコードでは安全な形になっている
- **一部修正**: 指摘の一部だけが解消している
- **未修正**: 指摘された処理が今もそのまま残っている
- **該当コードなし**: 対象のコードが削除・移管されている

「fork の状態」の列は、fork でその issue が閉じられているかと、修正 PR が手元に入っているかを示す。

## A. mesh crates（17 件）

| # | 深刻度 | 判定 | 根拠（手元コード） | 修正コミット / fork の状態 |
|---|---|---|---|---|
| #170 | High | 未修正 | `ingress/execution.rs:16` が公開 ID を内部名に書き換えてから route する。`response/routing.rs:92-97` の `priced_model_requires_payment` は書き換え後の名前を完全一致で引くだけ。一方 `payments/server.rs:125-133` の `priced_model` は requested と backend の両方の名前で引く。このため seller guard と paid server で照合キーが食い違う | なし |
| #76 | High | 未修正 | `stage/types.rs:156-162` の `StageStopRequest` に coordinator の本人性がない。`stage/mod.rs:401-445` の `stop()` は term/generation の大小しか見ない | fork では CLOSED。fork PR #94 は手元にない |
| #1 | High〜Medium | 未修正 | `request_parse.rs:799-806` は重複する Content-Length を後勝ちで上書きする。`:807-811` と `:837-839` は TE と CL が両方あると CL を黙って捨てる。`:740-746` は元のヘッダのまま転送する | fork では CLOSED。fork PR #21 は手元にない |
| #2 | Low | 未修正 | `request_parse.rs:788-856` はヘッダ長の上限を `Partial` 分岐でしか検査しない。chunked の wire 上限も、最後の read では効かない | fork では CLOSED。fork PR #21 は手元にない |
| #65 | Low〜Medium | 未修正 | ingress の `request_parse.rs:713-757` は hop-by-hop ヘッダや `Proxy-Authorization` を除去しない。peer への転送経路（`forwarded_request.rs:94-185`）では除去するが、これは以前からあるコードで issue への修正ではない | fork では CLOSED。fork PR #82 は手元にない |
| #68 | Medium | 一部修正 | upstream は別に実装し直している（`stream_translation.rs`、`responses_function_calls.rs`）。reasoning の保留と item ID の不一致は解消した。ただし `function_calls` の map、`arguments` / `output_text` の連結に上限がなく、DoS の項目は残っている | 8b848ead5（#2399、upstream が独自に再実装）。fork では CLOSED |
| #175 | Medium | 未修正 | `metrics.rs:1139-1147` の `redacted_label` は `policy.rs:178-180` の `is_http_url` を使うが、これは文字列の先頭からしか HTTP と判定しない。`plugin/health.rs:157-164` も trim せずに複製する | なし |
| #266 | Low〜Medium | 未修正 | `cli/parser/commands.rs:829` で token の検査関数を clap の `value_parser` として渡している。このため clap のエラーに元の値が入る。`mesh-llm/src/lib.rs:69-85` はそのエラーをそのまま表示する | なし |
| #145 | Low〜Medium | 未修正 | `keystore.rs:393-397` と `:452-468` は、ファイルから読んだ Argon2 の m/t/p を、アプリ側の上限なしで使う | なし |
| #176 | Low〜Medium | 未修正 | `payments/request.rs:31` は body を `Value` でしか保持しない。`canonical_digest.rs:69` が巨大な整数を検査するのは `source_json` があるときだけ | なし |
| #153 | Medium | 未修正 | `node_key.rs:142-157` は `symlink_metadata` で確認したあと、path を開き直す。`:44-47` の読み取りも同じ | なし |
| #192 | Medium | 未修正 | `logging/foundation.rs:299-310` は固定名 `.write_test` に `fs::write` する。create_new を使わず、symlink も区別しない | なし |
| #224 | Medium | 未修正 | `mesh-llm-skills/src/lib.rs:116-133` の公開関数 `install_skills` は skill 名を検証せず、`:200` で `root.join(&skill.name)` する | なし |
| #269 | Medium | 未修正 | `plugin_cli.rs:108-126` は、config で無効化された plugin でも store 側から解決して起動する | なし |
| #204 | Low | 未修正 | `tool_dispatch.rs:918-920` は名前を escape せずに `.` で連結する。登録（`:206`、`:238`、`:260`、`:286`）は `BTreeMap::insert` で、名前が衝突しても検出しない | なし |
| #264 | Low | 未修正 | `commands/download.rs:11-19` は catalog にない入力をそのまま analytics に渡す。`analytics/src/event.rs:93-110` は path 風の値を許す | なし |
| #315 | Low | 未修正 | `artifact_transfer_io.rs:50-75` と `:125-143` は partial ファイルの所有権を確保しない（create_new もロックも使わない） | なし |

## B. skippy crates（12 件）

| # | 深刻度 | 判定 | 根拠（手元コード） | 修正コミット / fork の状態 |
|---|---|---|---|---|
| #365 | High | 未修正 | `skippy-hf-hub/src/pagination.rs:68-71` は毎ページ同じ `auth_headers()` を付ける。`:115-125` の `parse_link_header_next` は origin を照合しない | なし |
| #363 | Medium | 修正済み | `repository/download.rs:537-541` で、ETag を `admit_etag` に、X-Repo-Commit を `admit_commit` に通してから path を作る。snapshot 経路と no_exist 経路も同じ | ef694d419（#2323、2026-10-12）。fork が分岐した後の upstream の変更で、fork には入っていない |
| #359 | Medium | 未修正 | `buckets/sync.rs:462` は remote path をそのまま rel として使う。`:689` で `local_path.join(&op.path)` し、`create_dir_all` する | なし |
| #384 | Medium | 未修正 | `skippy-native-runtime/src/cache.rs:111-112` は `root.join(release_version).join(native_runtime_id)` をそのまま返す。`manifest.rs:177` の検証は空文字の検査だけ | なし |
| #361 | Medium | 未修正 | `repository/download.rs:215-216` は、server が申告したサイズを上限なしで `BytesMut::with_capacity` に渡す | なし |
| #319 | Medium | 未修正 | `message_receive.rs:156` は「queue が 32MiB 未満か」を、読む前に見るだけ。1 frame は `MAX_STAGE_ACTIVATION_BYTES`（512MiB、`types.rs:19`）まで確保する | なし |
| #358 | Medium | 未修正 | `download.rs:730-757` は Range GET を mutable revision の URL に送る。If-Range も、応答の identity 照合もない | なし |
| #370 | Low〜Medium | 未修正 | `remote_catalog.rs:838-843` は `is_dir()` で symlink を辿って再帰し、JSON を削除する | なし |
| #373 | Low | 未修正 | `cache/storage.rs:280` と `:325` は symlink を辿る走査で、訪問済みの記録を持たない | なし |
| #372 | Low | 未修正 | `buckets/sync.rs:245-263` は month/day の範囲を検査せず、u64 で `d - 1` を計算する | なし |
| #371 | Low | 未修正 | `buckets/sync.rs:259` のガードは長さだけで、固定 byte 範囲で slice する | fork では CLOSED（PR #547、cf3f66fa3）。手元には入っていない |
| #339 | Low | 未修正 | `compose_mtp.rs:475-479` と `:866-867` は、ヘッダの count や長さを検証しないまま確保に使う | なし |

## C. CI・scripts・just・docs（13 件）

| # | 深刻度 | 判定 | 根拠（手元コード） | 修正コミット / fork の状態 |
|---|---|---|---|---|
| #474 | High | 未修正 | `tools/xtask/src/attestation.rs:430-439` の `inspect` は、status が missing/invalid でも `Ok` を返す。`scripts/ci-compose-product-input.sh:105-108` と `prepare-host-input/action.yml:112-115` は出力 JSON を見ない。`prepare-windows-host-input` は `$LASTEXITCODE` しか見ない | なし |
| #441 | Medium（High 寄り） | 未修正 | `just/mesh-client.just:4-14` は `{{ join }}` などを bash recipe の本文に埋め込む。`just/release-bundle.just:6` と `:10` も同じ | なし |
| #475 | Medium | 未修正 | `package-release.sh:533-556` の直接 packaging 経路は、`verify-native-runtime-package.sh` を通さない。`tools/xtask/src/product/compose.rs` は既存の checksum と照合しない | なし |
| #455 | Medium | 未修正 | 検証は `cargo xtool`（修復対象のツリーから `tools/xtask` をビルドする）に移った。しかし保護している pathspec は今も `.github .agents scripts evals ci` だけで（`agentic-replay-repair.sh:96`、`agentic-replay-nightly.yml:337`）、`tools/` も `.cargo/` も `Cargo.*` も保護されていない | なし（構成は変わったが問題は残る） |
| #459 | Medium | 一部修正 | smoke の Python 導入（`setup-canary-python/action.yml:12-17`）は step env で HF token を空にしている。`claude-live-model-gate.yml:24-26` の `ANTHROPIC_API_KEY` と、`ci-linux-product-smoke-slice.yml` の GPU job の `HF_TOKEN` は、今も job env にある | be4005c55（#2374、2026-10-12）。fork が分岐した後の upstream の変更 |
| #445 | Medium | 該当コードなし | `skippy/evals/wan-lab/` はディレクトリごと削除された。ただしルートの `.dockerignore` は今も `.env` などを除外しない | 496eec6a5（#2461、2026-10-17） |
| #451 | Medium | 未修正 | `compute-changes/derive-outputs.sh:427-429` は、今も固定の区切り `EOF` を使う | なし |
| #408 | Low | 修正済み | Python 版は削除された。Rust 版の `competitive_matrix.rs:246-277` の `safe()` は `.` と `..` と、安全な文字以外を拒否する。`ensure_directory`（`:357-374`）は symlink の親を拒否する | 967316821（#2360）、f5273fd1e（#2377）。どちらも fork が分岐した後の upstream の変更 |
| #270 | Low | 該当コードなし | `mesh/sdk/python/` は削除され、外部リポジトリ `Mesh-LLM/mesh-llm-python-sdk` に移った。checksum の検査が有効かは、外部リポジトリ側で確認する必要がある | f5273fd1e（#2377） |
| #456 | Low | 未修正 | `ci-control.yml:23` の判定は、今も `github.ref_name == default_branch` だけ | なし |
| #461 | Low | 修正済み | 固定 revision の外部 SDK の `requirements.lock` を、`--require-hashes` で導入している（`setup-canary-python/action.yml:38-50`） | be4005c55（#2374） |
| #498 | Low | 未修正 | `nightly-stability-run.yml:139` と `:144` は `@latest` を使う。`:149` は `stable` の installer を `curl \| bash` で実行する | なし |
| #541 | Low | 未修正 | `mesh/website/src/assets/mermaid.js:10` は、今も `mermaid@11` を読み込む | なし |
