# smoke の HF token をモデル取得の step にだけ渡す

- **実施日時**: 2026年11月02日 03:11 〜 05:12 JST (issue #59 の修正。smoke 系 4 job の token を job の env から input に移し、fallback を外し、xtask のガードを広げた)
- **報告日時**: 2026年11月02日 05:12 JST
- **作成者**: Claude Opus 5.5

## 概要

CI の smoke テストのうち 4 つの job では、Hugging Face の token を job 全体の環境変数に置いていました。この置き方だと、token を使わない step（第三者の action、クライアントの install、ビルド、mesh-llm の実行、後片付けなど）にも token が渡ります。どこか 1 つの step が乗っ取られたときに token まで漏れる場所が多い、という設計上の穴です。前回の #51 では Laya smoke だけを直したので、その残りを直しました。

まず、token を本当に使う step がモデル取得のほかに無いかを確かめました。smoke の本体は、取得済みのモデルファイルのパスを mesh-llm に渡して動かしているだけで、Hugging Face には取りに行きません。また、PR の CI ではもともと token が空のまま渡され、それでもこの 4 つの job は動いています。このことからも、本体は token が無くても動くと判断しました。

そのうえで、4 つの job の環境変数から token を外し、モデルを取得する step（`restore-smoke-inputs` と `restore-test-model` の呼び出し）にだけ input として渡すようにしました。あわせて、npm で OpenAI クライアントを入れる step にあった「token を空にする」設定も外しました。job 全体に token が無くなったので、もう必要ありません。

次に、`restore-test-model` が持っていた逃げ道を外しました。#51 では、input が空なら環境変数の token を使う、という fallback を残していました。まだ環境変数で渡している呼び出し元があったためです。すべての呼び出し元が input に移ったので、これを外しました。逃げ道が残っていると、誰かが job の環境変数に token を戻しても動いてしまい、後退に気づけません。

最後に、token の置き場所を検査する xtask のガードを広げました。新しいガードは、4 つの job について次のことを確かめます。job の環境変数に token が無いこと。モデル取得以外の step に token が現れないこと。モデル取得の step が `hf_token` という input でだけ token を渡していること。token を受け取らないほかの job（モデルは取得するが token を使わない job）が、token を渡し始めていないことも見ます。取得 step の中で、環境変数の token を読む fallback が戻っていないことも検査に加えました。

ローカルでは、ガードの単体テスト、xtask の CI 検証まわりのテスト、clippy、実リポジトリに対するガード全体の実行を行い、問題が無いことを確かめました。workflow を読む統合テストでは失敗がいくつか出ましたが、`just` が無いなどのワークステーションの環境によるものと、負荷がかかったときだけ落ちる時間依存のテストで、main でも同じように落ちることを確かめました。実際の CI での動作は、この fork に secret が無いため確かめられません。

## 添付ファイル

- [検証ログ](attachment/2026-11-02_033122_smoke_hf_token_input_scope/verify.log)
- [migration_lifecycle の失敗一覧（修正後）](attachment/2026-11-02_033122_smoke_hf_token_input_scope/migration_lifecycle_fix.txt)
- [migration_lifecycle の失敗一覧（`.github` と `tools` を main に戻した状態）](attachment/2026-11-02_033122_smoke_hf_token_input_scope/migration_lifecycle_main.txt)

## 前提・目的

- **背景**: issue #59。#51 / PR #58 の残課題。`smoke.yml`（`smoke_tests`、`smoke_tests_macos`）、`scripted-binary-smoke.yml`（`scripted_binary_smoke`）、`sdk-smoke.yml`（`sdk_smoke`）が、job の `env` に `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` を置いていた
- **目的**: token を `restore-test-model` の `Download integration model` にだけ届ける
- **前提条件**: secret が渡るのは trusted な push のときだけ（`ci-linux-lane.yml` などで `inputs.original_event_name == 'push' && secrets.HF_TOKEN || ''`）。この fork には secret も self-hosted の runner も無い

## 環境情報

- ブランチ: `fix/issue59-smoke-hf-token-input`（`origin/main` = `39d480ebc` から作成）
- 検証: ワークステーション、rustup 同梱の lld、`CARGO_TARGET_DIR` 共有

## 結果詳細

### token を使う step の確認

| job | token が無いと困る step | 根拠 |
|---|---|---|
| `smoke_tests` / `smoke_tests_macos` | モデル取得だけ | `scripts/ci-smoke-test.sh`、`scripts/ci-compat-smoke.sh` は取得済みモデルのパスを渡す。`HF_*` を参照しない |
| `scripted_binary_smoke` | モデル取得だけ | 呼び出し元が渡す `scripts/ci-two-node-client-serving-smoke.sh`、`scripts/ci-two-node-split-smoke.sh` は `HF_*` を参照しない |
| `sdk_smoke` | モデル取得だけ | `scripts/ci-{rust,kotlin,swift}-sdk-smoke.sh` は `$HOME/.models/...` を渡す。`HF_*` を参照しない |

- `scripts/` の中で `HF_TOKEN` を読むのは、`ci-hf-download-smoke.sh`（別 workflow の `hf-download-smoke.yml`）と `agentic-replay-repair.sh` だけ
- mesh-llm 本体は `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` を Hugging Face API クライアントの構築時に読む（`mesh-llm-host-runtime/src/models/mod.rs`）。ただ、smoke はローカルのファイルを渡すので、この経路は通らない
- PR の CI では、これら 4 つの job に空の token が渡り、それでも成功している

### 変更内容

| ファイル | 変更 |
|---|---|
| `.github/workflows/smoke.yml` | 2 job の `env` から token を削除。`restore-smoke-inputs` / `restore-test-model` の 4 か所に `hf_token: ${{ secrets.HF_TOKEN }}` を追加。npm の step の `HF_TOKEN: ""` を削除し、コメントを更新 |
| `.github/workflows/scripted-binary-smoke.yml` | job の `env` を削除。取得 step 2 か所に `hf_token` を追加 |
| `.github/workflows/sdk-smoke.yml` | job の `env` を削除。取得 step 1 か所に `hf_token` を追加 |
| `.github/actions/restore-test-model/action.yml` | `token="${MODEL_HF_TOKEN:-}"` にして、環境変数への fallback を削除。input の説明を更新 |
| `tools/xtask/src/ci_validation/workflow_guards/smoke_token.rs` | 新規。4 job の token の置き場所を検査するガードとテスト |
| `tools/xtask/src/ci_validation/workflow_guards/laya_token.rs` | `leaks` / `mentions` を `pub(super)` に。取得 step が `MODEL_HF_TOKEN` 以外から token を読まないことの検査（`reads_only_the_input`）とテストを追加 |
| `tools/xtask/src/ci_validation/workflow_guards.rs` | `smoke_token` を登録 |

### ガードが検査すること

- 4 つの job（`CARRIERS`）
  - job の `env` に `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` が無い
  - `restore-smoke-inputs` / `restore-test-model` を使わない step には、token の記述が無い（大文字小文字を区別しない）
  - それらを使う step は、すべて `with.hf_token: ${{ secrets.HF_TOKEN }}` を持ち、ほかの input・`env` では渡さない
- ほかの workflow で、`restore-smoke-inputs` / `restore-test-model` を使う job（今は `ci-linux-runtime-slice.yml` の `linux_runtime`、`ci-quality-slice.yml` の `skippy_cpp_api`）
  - job の `env` と step に token が無く、`hf_token` も渡さない
- `restore-test-model` の `Download integration model`
  - `run` が `MODEL_HF_TOKEN` 以外の名前で token を読まない。`env` に `MODEL_HF_TOKEN` 以外の token の束縛が無い

### 検証

| コマンド | 結果 |
|---|---|
| `cargo test -p xtask --bin xtask -- token` | 31 passed（新規 6 件を含む） |
| `cargo test -p xtask --bin xtask ci_validation` | 259 passed |
| `cargo clippy -p xtask --all-targets -- -D warnings` | 警告なし（初回は `redundant_closure` を 1 件指摘され、修正） |
| `cargo fmt -p xtask --check` | 差分なし |
| `cargo run -p xtask -- repo-consistency release-targets`（workflow ガード全体を実リポジトリに対して実行） | passed |
| `cargo run -p xtask -- ci validate-graph --workflows .github/workflows` | passed |
| `cargo test -p xtask --features automation-contracts --test migration_models` | 27 passed |
| `... --test migration_ci_graph` | 36 passed |
| `... --test ci_runner_cache_contract` | 254 passed、1 failed（既知。下記） |
| `... --test migration_lifecycle` | 修正後 85 failed、main 74 failed（下記） |

- `ci_runner_cache_contract` の `graph_actual_pr_and_main_event_census_keeps_exact_five_lane_entries` は、この fork が独自に足した `fork-ci.yml` で落ちる。前回（#51）のレポートと同じで、今回の変更とは関係ない
- `migration_lifecycle` の失敗の多くは、ワークステーションに `just` が無い、`MIGRATION_TEST_GIT` / `MESH_PYTHON_SDK_SOURCE` などの環境変数が無いことによる。`.github` と `tools` を `origin/main` に戻して同じテストを走らせ、失敗一覧を比べた。修正後だけで落ちた 11 件は、キャンセルやタイムアウトを扱う時間依存のテスト（`daemon::cli::d08_*`、`*_causal_*`、`runtime_events_gate::missing_or_empty_native_inputs_*` など）で、修正後の状態で個別に走らせると 14 件すべて通った。main 側だけで落ちたテストも 1 件ある。負荷による揺らぎで、今回の変更による失敗は無い

追加したテストでは、次の書き換えがそれぞれガードで落ちることを確かめた。

- 4 つの job の `env` に `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` を置く
- 4 つの job の、モデル取得以外のすべての step に `env.HF_TOKEN` を置く
- モデル取得の step で、`with.hf_token` を消す、`${{ github.token }}` に変える、`env` に token を置く、ほかの input で token を渡す
- token を受け取らない job（`ci-linux-runtime-slice.yml` の `linux_runtime`）の取得 step に `hf_token` を足す
- `Download integration model` の `run` に `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` への fallback を戻す。`env` に 2 つ目の token の束縛を足す

## 残課題

- 実際の CI（trusted な push で secret がある状態）での動作は、この fork では確かめられない
- jimoto-no-llm/mesh-llm#459 が挙げている Claude の API key（`claude-live-model-gate.yml`）の扱いは、この issue の範囲外
