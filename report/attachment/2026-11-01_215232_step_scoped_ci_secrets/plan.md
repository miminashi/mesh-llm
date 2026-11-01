# issue #22: CI secret を必要な step の env にだけ渡す

## Context

Claude live gate（`ANTHROPIC_API_KEY`）と Laya smoke（`HF_TOKEN`）は、secret を job 全体の `env:` に置いている。そのため、key を使わない step からも読める。たとえば npm の `install --global`、sccache action、`just automation-bootstrap`（`build.rs` や proc macro）、admit、cleanup などである。secret を、実際に使う step の `env:` にだけ移す。upstream と jimoto-no-llm には、2026-11-01 時点で修正提案がない（調査済み）。

調べて分かったこと: xtask のガード `tools/xtask/src/ci_validation/workflow_guards/claude_clients.rs:111-112`（`manual()`）は、**`ANTHROPIC_API_KEY` を job の env に置くことを強制している**。workflow だけを直すと、このガードと、そのテスト `claude_clients_tests.rs:116` が落ちる。そこで、ガードも「step に置くことを強制し、job には置かせない」形に直す。Laya のガード（`laya.rs`）は `HF_TOKEN` を見ていないので、新しく検査を足す。

## 作業場所

- worktree: `git worktree add -b fix/issue22-step-scoped-ci-secrets /home/ubuntu/projects/mesh-llm-worktrees/fix-issue22-step-scoped-ci-secrets origin/main`

## 変更内容

### 1. `.github/workflows/claude-live-model-gate.yml`
- job の `env:` から `ANTHROPIC_API_KEY` を削除する。`CARGO_*`、`MESH_*` はそのまま残す
- `ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}` を、次の 2 つの step の `env:` にだけ置く
  - `Require live Anthropic credential`
  - `Run live Claude model round trip through Mesh`
- `environment: claude-live-model`、client の固定（`2.1.273`）、`workflow_dispatch` だけで動くことは変えない

### 2. `.github/workflows/ci-linux-product-smoke-slice.yml`（laya_cpu / laya_cuda / laya_vulkan / laya_rocm）
- job の `env:` から `HF_TOKEN` を削除する。laya_vulkan の `MESH_LLM_VULKAN_AVAILABLE: '1'` は job に残す。laya_cpu / cuda / rocm は job の `env:` が空になるので、`env:` ごと消す
- `- uses: ./.github/actions/run-laya-product-smoke` の step に `env: HF_TOKEN: ${{ secrets.HF_TOKEN }}` を付ける。composite action の中の step（モデルを取得する `restore-smoke-inputs`）は、この env を引き継ぐ
- checkout、`Prepare source-bound native cleanup`、`Admit`、`Clean self-hosted Laya smoke outputs` には渡らなくなる。source-bound cleanup の流れそのものは変えない

### 3. `.github/workflows/ci-macos-product-smoke-slice.yml`（laya_metal）
- 同じ書き方が残っているので、同じように直す（issue に挙げた箇所には入っていないが、同じ種類の問題。PR 本文に書く）

### 4. ガード `tools/xtask/src/ci_validation/workflow_guards/claude_clients.rs`
- `manual()` の `h::binding(env, "ANTHROPIC_API_KEY", …)` を、次の検査に置き換える
  - job の env に `ANTHROPIC_API_KEY` が**無い**こと
  - 上の 2 つの step の env に、`${{ secrets.ANTHROPIC_API_KEY }}` がそのまま書かれていること
  - それ以外の step（install、accelerator、checkout など）の env に `ANTHROPIC_API_KEY` が無いこと
- `CARGO_PROFILE_TEST_DEBUG` の検査は job の env に残す

### 5. ガード `tools/xtask/src/ci_validation/workflow_guards/laya.rs`
- `row()`（Linux 4 行、macOS Metal、Windows CPU のすべての行に効く）に、次の検査を足す
  - job の env に `HF_TOKEN` が無いこと
  - `HF_TOKEN` を env に持つ step は、`run-laya-product-smoke` の step だけであること
  - Windows には `HF_TOKEN` 自体が無いので、この検査は「ある場合の置き場所」だけを縛る

### 6. テスト
- `claude_clients_tests.rs`
  - 既存の `manual_live_gate_requires_approval_environment_and_owned_secret_binding` が書き換えるパスを、job の env から step の env に変える
  - 次の 3 つの場合に `check()` がエラーになることを確かめるケースを足す
    - job の env に key を戻した場合
    - install の step に key を付けた場合
    - Run の step から key を消した場合
- `laya.rs` の tests: `HF_TOKEN` を job の env に戻した場合と、cleanup の step に付けた場合に、エラーになることを確かめる
- 実物の workflow を読むテスト（`actual_required_and_manual_claude_workflows_retain_real_client_contract` と、laya の実物の workflow を読むテスト）が通ること

## 検証

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-issue22-step-scoped-ci-secrets
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -j 8 --locked -p xtask --bin xtask ci_validation::workflow_guards
cargo run -j 8 -p xtask -- repo-consistency   # docs_and_workflow_invariants を含む全体検査（サブコマンドは実行時に確認する）
python3 -I -c 'import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]' .github/workflows/{claude-live-model-gate,ci-linux-product-smoke-slice,ci-macos-product-smoke-slice}.yml
grep -n 'HF_TOKEN\|ANTHROPIC_API_KEY' 上の 3 ファイル   # job の直下に残っていないことを目で確かめる
git diff --check
```

- actionlint はワークステーションに無い。PR の CI で走るなら、その結果を見る
- 実際に secret を使う実行（live gate、GPU runner）は、この fork には secret も runner も無いので確かめられない。そのことを PR とレポートに書く

## コミット / PR / レポート

- コミット 1: `fix(ci): scope CI secrets to the steps that use them (#22)`（workflow とガードとテスト）。Co-Authored-By のトレーラーは付けない（memory の規則）
- コミット 2: `docs(report): ...` レポートを worktree の `report/` に置く。プランを `attachment/<名前>/plan.md` に添付する
- push して、`gh pr create --repo miminashi/mesh-llm` で PR を作る。本文に `Closes #22`、macOS Metal も直したこと、検証できなかったことを書く
- statusline のファイル `~/.claude/statusline-session/49233d9d-…` を `issue #22 | PR #NN` に更新する
