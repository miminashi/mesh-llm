## 概要

OpenAI 互換 ingress が、あいまいな HTTP framing（`Content-Length` の重複、`Transfer-Encoding` と `Content-Length` の併存など）を受理していた問題を直します。jimoto-no-llm/mesh-llm の PR #21 の 3 コミットを cherry-pick したものです。

Closes #5

## 変更内容

- `request_parse/framing.rs`（新規）で、body を読む前に framing を 1 つに決める
  - `Content-Length` の重複（同じ値でも、カンマ区切りでも）は拒否する
  - 受け付ける transfer coding は `chunked` 1 つだけ。`chunked` が最後にない、ほかの coding が付いている、パラメータが付いている、といった場合は拒否する
  - `Content-Length` と `Transfer-Encoding` が両方あれば拒否する
  - 拒否すると、既存の `send_read_failure` の経路で 400 を返す
- 転送時は、元の `Content-Length` / `Transfer-Encoding` を必ず捨て、framing を 1 つだけ付け直す
- ヘッダが 1 回の読み込みで届き切った場合も、ヘッダの上限を検査する。転送用に作り直したヘッダも同じ上限で検査する
- chunked 本文の上限は、そのメッセージが実際に消費したバイト数で判定する（届き切った読み込みも数え、後続リクエストの先読み分は数えない）

## 取り込み時の調整

1 つ目のコミットは、PR #21 の後に upstream で入った `/v1/realtime` の upgrade 判定（`Upgrade` / `Connection` ヘッダの読み取り）とコンフリクトしました。両方を残す形で解消しています。

## 検証

- `cargo test -p mesh-llm-host-runtime --lib openai::request_parse`: 84 passed
- `cargo test -p mesh-llm-host-runtime --lib`: 4211 passed / 1 failed
  - 失敗した `runtime::tests::startup_models::layer_diagnostics::unresolved_layer_package_ref_surfaces_actionable_diagnostic` は、単独で再実行するとこのブランチでも main でも pass する（並列実行時だけ落ちる不安定なテストで、今回の変更とは無関係）
- `cargo test -p mesh-llm-host-runtime --test '*'` と `--doc`: すべて pass
- `cargo clippy -p mesh-llm-host-runtime --all-targets --no-deps -- -D warnings`: pass
- `cargo fmt --check -p mesh-llm-host-runtime`: pass
- `cargo xtool repository affected-crates`: `test_crates` は `mesh-llm-host-runtime` のみ

ワークステーションには sccache が無いため、`RUSTC_WRAPPER=` と `MESH_LLM_SKIP_UI=1` を付けて実行しました。

🤖 Generated with [Claude Code](https://claude.com/claude-code)
