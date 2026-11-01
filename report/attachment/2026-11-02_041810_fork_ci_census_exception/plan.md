# issue #60: fork-ci.yml をワークフロー数え上げテストの例外に足す

## Context

`ci_runner_cache_contract` の `graph_actual_pr_and_main_event_census_keeps_exact_five_lane_entries` は、`pull_request` で動く workflow から例外 3 つを除いたものが upstream の 5 lane（`pr_{quality,website,linux,macos,windows}.yml`）と一致することを確かめる。fork 独自の `fork-ci.yml` が 6 本目として数えられ、fork では失敗する。upstream と jimoto-no-llm に修正の提案は無い（issue コメントで確認済み）。ユーザーは「例外に `fork-ci.yml` を足す」案を選んだ。

## 手順

1. worktree を作る（メインチェックアウトでは switch しない）
   ```bash
   git fetch origin
   git worktree add -b fix/issue60-fork-ci-census-exception \
     /home/ubuntu/projects/mesh-llm-worktrees/fix-issue60-fork-ci-census-exception origin/main
   ```
2. ステータスライン: `echo "issue #60" > ~/.claude/statusline-session/97add07a-db6d-4d6e-92d9-192a23be40b4`（PR 作成後に `issue #60 | PR #N`）
3. 修正: `tools/xtask/tests/ci_runner_cache_contract/artifact_workflows/declared_graph.rs` の `exceptions`（`BTreeSet::from([...])`）に `"fork-ci.yml".to_owned(),` を 1 行足す。upstream との衝突を小さくするため、既存行は動かさず末尾に追加する。fork 独自であることが分かるよう短いコメントを添える
   - `assert!(exceptions.is_subset(&pr))` があるので、`fork-ci.yml` が `pull_request` で動かなくなったらテストが落ちて気づける（例外が古くなるのを防げる）
4. 検証（CLAUDE.md のメモリ対策つき）
   ```bash
   export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
   export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
   cargo fmt --all --check
   cargo test -j 8 -p xtask --features automation-contracts --test ci_runner_cache_contract
   ```
   - 対象のテストが通り、`ci_runner_cache_contract` の他のテストが壊れていないことを確認する
   - 余裕があれば `cargo test -j 8 -p xtask --features automation-contracts` で、fork 独自 workflow で落ちる他の契約テストが無いかも見る。あれば結果を issue に書くだけにして、この PR の範囲は広げない
5. コミット（`test(xtask): count the fork's fork-ci.yml as a PR workflow exception` のような Conventional Commits の件名）。**Co-Authored-By トレーラーは付けない**（fork の Commit convention CI が拒否する）
6. レポート: worktree の `report/<JST タイムスタンプ>_fork_ci_census_exception.md` を CLAUDE.md の書式で書き、本プランを `report/attachment/<名前>/plan.md` に添付。別コミット `docs(report): ...`
7. push して PR を作る（タイトルは Conventional Commits、本文に `Closes #60`、末尾に Claude Code の行）。ステータスラインを PR 番号つきに更新

## 変更するファイル

- `tools/xtask/tests/ci_runner_cache_contract/artifact_workflows/declared_graph.rs`（1〜2 行）
- `report/` 配下（レポートとプランの添付）
