# Laya smoke の HF token をモデル取得の step にだけ渡す

- **実施日時**: 2026年11月02日 00:59 〜 01:30 JST (issue #51 の修正。composite action の入れ子を input で token を受け渡す形に直し、xtask のガードを書き換え)
- **報告日時**: 2026年11月02日 01:30 JST
- **作成者**: Claude Opus 5.5

## 概要

CI の Laya smoke では、Hugging Face からモデルを取得するために token を使います。前回の修正（#22）で、この token を job 全体から外し、Laya smoke を実行する 1 つの step にだけ渡すようにしました。ところが、その step が呼んでいるのは composite action という「いくつもの step をまとめた部品」で、部品の中の step は呼び出し側の環境変数をそのまま引き継ぎます。そのため、部品の中にある、token を使わないビルドや第三者の action、mesh-llm 本体の実行にも、token が渡ったままでした。

今回は、token を環境変数で上から配るのをやめて、部品の引数（input）として 1 段ずつ手渡しする形に変えました。呼び出し側は input として渡し、中間の 2 つの部品はそれを次の部品へ input で渡すだけにして、いちばん奥の「モデルを取得する step」でだけ環境変数に置きます。input は、明示的に書いた場所にしか届かないので、ほかの step には漏れません。

ほかの workflow も同じ部品を使っていて、そちらは今も job 全体の環境変数に token を置いています。それらを壊さないように、input が空のときは今までどおり環境変数の token を使うようにしました。

あわせて、token の置き場所を検査している xtask のガードを書き直しました。これまでのガードは「job と、呼び出し以外の step に token が無いか」しか見ておらず、部品の中は検査していませんでした。新しいガードは、呼び出し側から部品の奥まで受け渡しの経路をたどり、取得の step 以外のどこにも token が現れないことを確かめます。誰かが元の書き方に戻したり、別の step に token を足したりすると、ガードが落ちます。

テスト、clippy、リポジトリ全体の整合性チェックは通りました。CI 用の統合テストに 1 件失敗がありますが、この fork が独自に足した workflow を数えてしまうことによるもので、今回の変更とは関係なく main でも同じように落ちます。

この fork には secret も GPU の runner も無いので、実際に token を使った取得は試せていません。確認は、YAML の解析とガードのテストによるものです。

## 添付ファイル

なし（plan mode を使っていないため、実装プランは無い）

## 前提・目的

- **背景**: #22 / PR #47 の残課題（[前回のレポート](./2026-11-01_215232_step_scoped_ci_secrets.md)の「残課題」）。issue #51 で、composite action の中で token が不要な step にも渡っていることを指摘した
- **目的**: `HF_TOKEN` を `restore-test-model` の `Download integration model` にだけ渡す。issue の「案 1（input で渡す）」で直す
- **前提条件**: `restore-smoke-inputs` / `restore-test-model` を呼ぶ他の workflow（`smoke.yml` など）の挙動は変えない

## 環境情報

- ブランチ: `fix/issue51-hf-token-input`（`origin/main` = `224b0d71f` から分岐）
- ワークステーション、rustup stable、リンカーは rustup 同梱の lld

## 結果詳細

### token の受け渡し経路

```text
呼び出し側 step
  with: hf_token: ${{ secrets.HF_TOKEN }}         ← env には置かない
└─ run-laya-product-smoke      (input hf_token → restore の with へ)
   └─ restore-smoke-inputs     (input hf_token → resolve-model の with へ)
      └─ restore-test-model    (input hf_token)
         └─ Download integration model
              env: MODEL_HF_TOKEN: ${{ inputs.hf_token }}   ← ここにだけ置く
```

`Download integration model` の token の決め方は `"${MODEL_HF_TOKEN:-${HF_TOKEN:-${HUGGING_FACE_HUB_TOKEN:-}}}"`。input が空なら、従来どおり環境変数の `HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` を使う。

### 変更したファイル

| ファイル | 変更 |
|---|---|
| `.github/actions/run-laya-product-smoke/action.yml` | input `hf_token` を追加し、`restore` step の `with` に渡す |
| `.github/actions/restore-smoke-inputs/action.yml` | input `hf_token` を追加し、`resolve-model` step（`restore-test-model`）の `with` に渡す |
| `.github/actions/restore-test-model/action.yml` | input `hf_token` を追加し、`Download integration model` の `env` に `MODEL_HF_TOKEN` として置く。空なら環境変数に fallback |
| `.github/workflows/ci-linux-product-smoke-slice.yml` | laya_cpu / laya_cuda / laya_vulkan / laya_rocm の呼び出しを `env: HF_TOKEN` から `with: hf_token` に変更 |
| `.github/workflows/ci-macos-product-smoke-slice.yml` | laya_metal の呼び出しを同様に変更 |
| `tools/xtask/src/ci_validation/workflow_guards/laya_token.rs` | 新規。token の経路を検査するガードとテスト |
| `tools/xtask/src/ci_validation/workflow_guards/laya.rs` | 旧 `model_token_scope()` を削除して `laya_token` を呼ぶ。workflow 側のテストを追加 |

Windows の laya_cpu には、もともと token が無いので変更していない。

### ガードが検査すること（`laya_token.rs`）

workflow 側（`caller_scope`）:

- job の `env` に token が無い
- 呼び出し step 自身を含め、どの step の `env` にも token が無い
- 呼び出し以外の step には、token に関する記述がどこにも無い
- 呼び出し step は、`with.hf_token: ${{ secrets.HF_TOKEN }}` でだけ渡す（ほかの input で渡さない）
- Windows の行は `hf_token` を渡さない

action 側（`action_chain`）:

- 3 つの action に、`required: false`、`default: ''` の input `hf_token` がある
- `run-laya-product-smoke` は `restore` step に、`restore-smoke-inputs` は `resolve-model` step に、`with.hf_token: ${{ inputs.hf_token }}` でだけ渡す（`env` には置かない。`secrets` を直接参照しない）
- `restore-test-model` の `Download integration model` の `env` に `MODEL_HF_TOKEN: ${{ inputs.hf_token }}` がある
- 上の受け渡し以外の step には、`HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN` / `hf_token` の記述が無い（大文字小文字を区別しない）

### テスト

| コマンド | 結果 |
|---|---|
| `cargo test -p xtask --bin xtask laya` | 24 passed（新規 4 件、拡張 1 件を含む） |
| `cargo test -p xtask --bin xtask ci_validation` | 253 passed |
| `cargo clippy -p xtask --all-targets -- -D warnings` | 警告なし |
| `cargo run -p xtask -- repo-consistency release-targets`（workflow ガード全体を実リポジトリに対して実行） | passed |
| `cargo run -p xtask -- ci validate-graph --workflows .github/workflows` | passed |
| `cargo test -p xtask --features automation-contracts --test migration_models --test migration_lifecycle --test ci_runner_cache_contract --test migration_ci_graph` | 1 failed（下記）、ほかは passed |

失敗した `graph_actual_pr_and_main_event_census_keeps_exact_five_lane_entries` は、PR で動く workflow の一覧に、この fork が独自に足した `fork-ci.yml` が入っていることで落ちる。今回の変更とは関係なく、main でも同じように落ちる。

追加したテストでは、次の書き換えがそれぞれガードで落ちることを確かめた。

- 部品の中の、token を使わない step（`Run Laya startup and golden reads`、`Prepare hosted smoke automation`、`Extract and verify composed product`、`Verify integration model integrity` など）に、`env.HF_TOKEN` / 別名の `env` / `with` で token を足す
- 受け渡しの step で、`env` に token を置く、`with.hf_token` を消す、`secrets.HF_TOKEN` を直接渡す
- `Download integration model` から `MODEL_HF_TOKEN` を消す
- input を `required: true` にする、既定値を空以外にする
- 呼び出し step の `env` に token を置く、`with.hf_token` を消す、Windows の行に `hf_token` を足す

## 残課題

- secret と GPU の runner が無いため、実際に token を使ったモデル取得は CI で確かめていない
- `smoke.yml` や `scripted-binary-smoke.yml` など、`restore-smoke-inputs` / `restore-test-model` を呼ぶ他の workflow は、今も job 全体の `env` に `HF_TOKEN` を置いている。同じ input で渡すように揃えれば、fallback を外せる。範囲が広いので、必要なら別の issue にする
- `ci_runner_cache_contract` の統合テストが `fork-ci.yml` で落ちる件は、この fork の CI 設定の問題として別に扱う
