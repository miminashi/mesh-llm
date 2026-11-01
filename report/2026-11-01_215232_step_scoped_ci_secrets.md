# CI の secret を使う step にだけ渡すよう修正

- **実施日時**: 2026年11月1日 21:20 〜 22:05 JST (issue #22 の修正: workflow で secret を置く場所を変え、それを強制する xtask ガードを直した)
- **報告日時**: 2026年11月1日 22:05 JST
- **作成者**: Claude Opus 5.5

## 概要

issue #22 は、CI の一部の job が秘密情報（Anthropic の API key と Hugging Face の token）を job 全体の環境変数に置いているという指摘です。job 全体に置いた環境変数は、その job のすべての step から読めます。そのため、key を使わない step でも、外部パッケージの install やビルドスクリプトの中で key を読めてしまいます。

着手する前に、upstream と、この問題を最初に見つけた jimoto-no-llm の fork を確認しました。どちらにも修正の提案は出ておらず、該当する箇所も同じまま残っていました。そこで、この fork で直すことにしました。

修正では、secret を job 全体の環境変数から外し、実際に使う step の環境変数にだけ置きました。Claude live gate では、key があるかを確かめる step と、Claude の API を呼ぶテストの step の 2 つです。Laya smoke では、モデルを取得する smoke action を呼ぶ step だけです。issue に挙がっていた Linux の 4 つの job のほかに、macOS の Metal 用 job にも同じ書き方が残っていたので、あわせて直しました。

調べてみると、リポジトリの xtask には workflow の書き方を検査するガードがありました。このガードは、Claude の key を job 全体に置くことを求めていました。workflow だけを直すと、このガードで CI が落ちます。そこでガードも書き換え、key を必要な step にだけ置くことを求め、job 全体や他の step に置くと失敗するようにしました。Laya 側にはこうした検査がなかったので、新しく足しました。今後、upstream を取り込んだときなどに元の書き方に戻ってしまっても、ガードが知らせます。

確認として、ガードのテストをすべて通しました。修正前の workflow を読ませると、新しいガードが失敗することも確かめました。一方、実際に secret を使う実行（live gate や GPU runner での smoke）は、この fork には secret も runner もないので試せていません。

## 添付ファイル

- [実装プラン](attachment/2026-11-01_215232_step_scoped_ci_secrets/plan.md)

## 前提・目的

- **背景**: issue #22（jimoto-no-llm/mesh-llm#459 由来）。`smoke.yml` の pip や npm の step については、以前の修正で token を空にしてある。この PR では、残っている Claude live gate と Laya smoke を直す
- **目的**: secret を必要な step にだけ渡す。承認 environment（`claude-live-model`）、client のバージョン固定（`2.1.273`）、push のときだけ secret を渡す呼び出し側の条件、GPU runner の source-bound cleanup は変えない
- **upstream の状況（2026-11-01 確認）**: Mesh-LLM/mesh-llm の main（`e7e33fedf`）にも、jimoto-no-llm/mesh-llm の main（`8959535ab`）にも、同じ書き方が残っている。関係する PR や issue は見つからなかった（jimoto-no-llm#459 は OPEN のまま、コメントなし）

## 環境情報

- ブランチ: `fix/issue22-step-scoped-ci-secrets`（作成元は `origin/main` の `591e7d015`）
- 確認したマシン: ワークステーション（lld でリンク、`RUSTC_WRAPPER=` で sccache を外した）

## 結果詳細

### workflow の変更

| ファイル | job | 変更前 | 変更後 |
|---|---|---|---|
| `claude-live-model-gate.yml` | `live_claude` | job の env に `ANTHROPIC_API_KEY` | `Require live Anthropic credential` と `Run live Claude model round trip through Mesh` の 2 つの step の env にだけ置く |
| `ci-linux-product-smoke-slice.yml` | `laya_cpu` / `laya_cuda` / `laya_vulkan` / `laya_rocm` | job の env に `HF_TOKEN` | `uses: ./.github/actions/run-laya-product-smoke` の step の env にだけ置く。laya_vulkan の `MESH_LLM_VULKAN_AVAILABLE` は job に残す |
| `ci-macos-product-smoke-slice.yml` | `laya_metal` | 同上 | 同上（issue には挙がっていなかった箇所） |

composite action に渡した step の env は、その中の step（`restore-smoke-inputs` から `restore-test-model` へ渡ってモデルを取得する step）が引き継ぐ。checkout、`Prepare source-bound native cleanup`（`just automation-bootstrap`）、`Admit cleanup controller`、`Clean self-hosted Laya smoke outputs` には渡らなくなる。

### xtask ガードの変更

- `tools/xtask/src/ci_validation/workflow_guards/claude_clients.rs`: `credential_scope()` を追加した。job の env に key があれば失敗し、上の 2 つの step に `${{ secrets.ANTHROPIC_API_KEY }}` がそのまま書かれていなければ失敗し、それ以外の step に key があれば失敗する。これまでは逆に、job の env に置くことを求めていた
- `tools/xtask/src/ci_validation/workflow_guards/laya.rs`: `model_token_scope()` を追加し、すべての Laya 行（`row()`）で、job の env と、smoke action 以外の step の env に `HF_TOKEN` があれば失敗するようにした。Windows CPU の行にはもともと token がないので、この検査は「置く場合の場所」だけを縛る
- テストを追加した
  - `live_credential_is_bound_only_on_its_check_and_live_test_steps`: key の値のすり替え、key の欠落、job 全体への再配置、install / accelerator の step への追加を、すべて弾くことを確かめる
  - `model_token_reaches_only_the_laya_smoke_action_step`: 5 つの job それぞれで、job の env と、action 以外のすべての step に token を足すと弾くことを確かめる

### 検証

| コマンド | 結果 |
|---|---|
| `cargo test -p xtask --bin xtask ci_validation::workflow_guards` | 189 passed |
| 上と同じテストを修正前の workflow で（`git stash` で workflow だけ戻して）実行 | 実物の workflow を読むテスト 2 本が、`live Claude credential must not be bound on the whole job` と `Laya smoke must not bind HF_TOKEN on the whole job` で失敗した（期待どおり） |
| `cargo run -p xtask -- repo-consistency release-targets`（`just check-release` と同じ。workflow の不変条件の検査を含む） | passed |
| `cargo clippy -p xtask --all-targets -- -D warnings` | 警告なしで通った |
| PyYAML での構文チェック、`git diff --check` | ok |

## 残課題

- secret を実際に使う実行は確かめていない。この fork には secret も environment も self-hosted の GPU runner もないため。step の env に置いた secret が composite action の中の step に引き継がれるのは GitHub Actions の仕様どおりだが、実際の実行で確かめたわけではない
- `run-laya-product-smoke` の中では、モデルを取得したあとの `Run Laya startup and golden reads`（mesh-llm 本体を動かす step）にも token が引き継がれる。この step はローカルのモデルを使うので token は要らないはずだが、確かめていないので空にしなかった。もっと絞り込むなら、`smoke.yml` と同じように、その step で `HF_TOKEN: ''` にする
- actionlint はワークステーションにないので、手元では実行していない
