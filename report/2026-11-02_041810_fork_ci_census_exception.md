# fork-ci.yml を PR workflow 数え上げテストの例外に追加

- **実施日時**: 2026年11月2日 03:13 〜 04:20 JST (issue #60 の修正: ci_runner_cache_contract の数え上げテストに fork-ci.yml の例外を追加し、xtask 契約テスト全体を確認)、2026年11月3日 12:00 〜 12:40 JST (同じ原因で落ちていた 2 件の契約テストを fork に合わせて修正)
- **報告日時**: 2026年11月3日 12:40 JST
- **作成者**: Claude Opus 5.5

## 概要

upstream には、CI の構成が設計どおりかを確かめる契約テストがあります。そのひとつは、PR で動く workflow を数え、雑用系の例外を除くとちょうど upstream の 5 本の lane になることを確かめます。この fork では、upstream の lane を動かせない代わりに `fork-ci.yml` を足しているので、テストはこれを想定外の 6 本目と見なして失敗していました。

upstream と、別の fork である jimoto-no-llm/mesh-llm を調べましたが、この件の修正提案はありませんでした。upstream には `fork-ci.yml` が無いので、upstream が自分から直すことはないと考えられます。そこでユーザーと相談して、テストの例外リストに `fork-ci.yml` を 1 行足す方法に決めました。

修正は、既存の例外の末尾に 1 行とコメントを足しただけです。upstream を取り込むときの衝突は小さく済みます。テストには「例外がすべて実際に PR で動いている」ことを確かめる行もあるので、将来 `fork-ci.yml` が PR で動かなくなれば、テストが落ちて例外が古くなったことに気づけます。

修正後、対象のテストバイナリ（255 件）はすべて通りました。修正前は、この 1 件だけが落ちていました。

ついでに、`automation-contracts` feature を付けて xtask のテストを全部流しました。100 件を超える失敗が出ましたが、ほとんどはこのワークステーションの環境が原因です。必要なツールや環境変数が無い、負荷のせいでタイムアウトした、などで、今回の修正とは関係ありません。

ただし、その中に、#60 と同じく fork が workflow を書き換えたせいで落ちるテストが 2 件ありました。どちらも、upstream 専用のジョブに fork が付けた `github.repository == 'Mesh-LLM/mesh-llm'` の条件が、テストの期待する `if:` の文字列と食い違うものです。いったん issue #60 に記録したあと、ユーザーの判断で、この 2 件も同じ PR でテストの期待値を fork の条件に合わせました。

その作業の前に最新の main を取り込んだところ、canary の契約テストが新たに 4 件落ちるようになりました。調べると、upstream で canary の workflow を書き換えた変更が、契約テストを更新しないままマージされたためでした。fork の変更とは関係なく、upstream ではすでに修正の PR が出ているので、この PR では手を付けず、upstream の修正を取り込むのを待ちます。

## 添付ファイル

- [実装プラン](attachment/2026-11-02_041810_fork_ci_census_exception/plan.md)
- [xtask 契約テスト全体の結果（テストバイナリごとの集計と失敗したテスト名）](attachment/2026-11-02_041810_fork_ci_census_exception/xtask-automation-contracts-summary.log)

## 核心発見サマリ

**結論**: `declared_graph.rs` の `exceptions` に `"fork-ci.yml"` を足すと、`ci_runner_cache_contract` は 254 passed / 1 failed から 255 passed / 0 failed になった。xtask の `automation-contracts` テスト全体（123 件失敗）のうち、fork の workflow 変更が原因と特定できたのは、ほかに 2 件（`pr_sibling_cancellation`、`canary_workflow_contract`）。残りは環境とタイムアウトによるもの。

## 前提・目的

- **背景**: issue #60。PR #58 の作業中に、`automation-contracts` feature 付きのテストで見つかった。fork の CI はこの feature を付けたテストを流していない
- **目的**: fork が意図して足した `fork-ci.yml` を、数え上げテストの例外として扱う
- **前提条件**: この fork は upstream と同じ CI を目指していない。upstream との差分は小さく保つ

## 環境情報

- ワークステーション（32 コア、メモリ 31GB）、rustup 同梱の lld でリンク、`RUSTC_WRAPPER=` で sccache を外す
- ブランチ: `fix/issue60-fork-ci-census-exception`（`origin/main` 39d480ebc から作成）
- worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue60-fork-ci-census-exception`

## 再現方法

```bash
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo fmt --all --check
cargo test -j 8 -p xtask --features automation-contracts --test ci_runner_cache_contract
# 参考: 契約テスト全体
cargo test -j 8 -p xtask --features automation-contracts --no-fail-fast
```

## 結果詳細

### 修正内容

`tools/xtask/tests/ci_runner_cache_contract/artifact_workflows/declared_graph.rs` の `graph_actual_pr_and_main_event_census_keeps_exact_five_lane_entries`:

```rust
    let exceptions = BTreeSet::from([
        "pr_auto_assign.yml".to_owned(),
        "pr_cleanup.yml".to_owned(),
        "pr_ci_canary.yml".to_owned(),
        // miminashi/mesh-llm fork: runs on PRs in place of the upstream-only lanes.
        "fork-ci.yml".to_owned(),
    ]);
```

### ci_runner_cache_contract

| | passed | failed | 失敗したテスト |
|---|---|---|---|
| 修正前 | 254 | 1 | `graph_actual_pr_and_main_event_census_keeps_exact_five_lane_entries` |
| 修正後（単独で実行） | 255 | 0 | なし |

### xtask の automation-contracts 全体（修正後、`--no-fail-fast`）

失敗は合計 123 件。主な内訳:

- **fork の workflow 変更が原因（2 件）**: 下の「副次発見」を参照
- **ツールや環境変数が無い**: `MIGRATION_TEST_GIT` 未設定（`compute_changes_justfiles` の 11 件）、PATH に fixture 用のツールが無い（`lld_shell_contract`、`macos_deployment_target`、`source_layout_action` など）、Swift / Kotlin の SDK が無い（`swift_privacy_adapter` の 17 件、`native_sdk_restore` など）
- **負荷によるタイムアウト**: `ProcessReport { outcome: Deadline, ... }` で落ちたもの。`ci_runner_cache_contract` の 5 件もこれで、単独で実行したときは全件通った

### 追加修正: 同じ原因の契約テスト 2 件（2026-11-03）

fork のジョブ条件 `github.repository == 'Mesh-LLM/mesh-llm' && ...` を期待値にした:

- `tools/xtask/tests/migration_lifecycle/pr_sibling_cancellation.rs`: `if:` を空白で区切ったトークン列の先頭に `github.repository` / `==` / `'Mesh-LLM/mesh-llm'` / `&&` を足した
- `tools/xtask/tests/migration_lifecycle/canary_workflow_contract.rs`: `alert-consecutive-failures` の `if:` の期待文字列に同じ条件を足した

`cargo test -j 8 -p xtask --features automation-contracts --test migration_lifecycle -- pr_sibling_cancellation canary_workflow_contract` の結果、対象の 2 件を含む 18 件が通った。失敗した 4 件は下の upstream 起因のもの。

### upstream 起因の canary 契約テストの失敗（4 件）

最新の origin/main（upstream 取り込み済み）をマージしたあとに出た。upstream #2811（Make the llama upgrade canary platform independent）が `llama-canary-family-pass.yml`、`llama-upstream-canary.yml`、`setup-canary-runner` などを書き換えたが、`canary_workflow_contract.rs` を更新していない:

| テスト | 失敗内容 |
|---|---|
| `actual_setup_action_preserves_local_git_identity_and_llvm_fallback_refusal_without_installing` | `Verify runner toolchain` ステップが見つからない |
| `parsed_family_worker_preserves_bounded_fanout_and_prebuilt_restore_before_certification` | 期待したキーが無い（`field` の unwrap） |
| `frozen_prebuilt_workers_load_cache_after_controller_and_before_consumption` | `runner-profile` が `preinstalled-macos` ではなく `${{ needs.resolve.outputs.runner_profile }}` |
| `protected_controller_and_worker_credentials_refuse_permission_or_token_mutations` | `publication credential key in worker/controller node` |

upstream の main は、この組み合わせで赤くなっている。修正 PR Mesh-LLM/mesh-llm#2814（fix: repair main after the bindgen, canary and split DFlash merges、open）が `canary_workflow_contract.rs` を直している。

## 副次発見

fork が upstream 専用のジョブに `github.repository == 'Mesh-LLM/mesh-llm'` の条件を足した（コミット f5c517015）ことで、`if:` の文字列を厳密に比べる次の契約テストも落ちている:

- `migration_lifecycle::pr_sibling_cancellation::sibling_monitor_is_protected_and_pr_entrypoints_have_no_actions_authority`（`.github/workflows/pr-cancel-sibling-runs.yml` の `monitor` ジョブ）
- `migration_lifecycle::canary_workflow_contract::actual_scheduled_alert_reconciles_first_failure_repeated_failure_and_recovery_with_finite_api`（`.github/workflows/llama-upstream-canary.yml` の `alert-consecutive-failures` ジョブ）

## 残課題

- 上の 2 件は、2026-11-03 にこの PR で修正済み
- upstream #2814 がマージされたら取り込み、canary 契約テストの 4 件が通ることを確かめる。#2814 が alert ジョブの `if:` の期待値も変えていれば、fork の条件と合わせ直す
