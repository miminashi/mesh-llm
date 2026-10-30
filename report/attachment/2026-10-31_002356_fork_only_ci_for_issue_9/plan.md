# Issue #9: fork で動かない upstream CI を止め、fork 専用 CI に置き換える

## Context

この fork（miminashi/mesh-llm）では、upstream の非公開イメージ `ghcr.io/mesh-llm/mesh-llm-cuda-runner` を使うジョブがすべて `manifest unknown` で失敗する。issue #9 の「方針（2026-10-30 追記）」で、次の 2 点が決まっている。

1. upstream 由来の PR／main／スケジュールのワークフローは、`github.repository == 'Mesh-LLM/mesh-llm'` を条件にして fork では動かさない
2. 代わりに `ubuntu-24.04` で直接動く fork 専用ワークフロー `.github/workflows/fork-ci.yml` を追加し、fmt・Commit convention・clippy・Rust テストを実行する

これに沿って実装する。upstream との競合を減らすため、既存ファイルへの変更は `if:` だけにとどめ、fork 専用の処理は新しいファイルにまとめる。

## 調査で分かったこと（設計に効くもの）

- GitHub Actions には**ワークフロー全体に付ける `if:` がない**。ジョブ単位で付ける必要がある。`pr_*.yml` / `main_*.yml` は `plan → lane → required` の構成で、`required` が `if: ${{ !cancelled() }}` なので、`plan` を止めただけでは `required` が走り、`PLAN_RESULT != success` で fail する。そのため、**`plan` と `required` の 2 か所**に条件を付ける（`lane` は plan の成功が前提なので自動的に skip される）
- `llama-upstream-canary.yml` は `resolve` にはすでに条件があるが、`result`（`!cancelled()`）が走って fail している（run 41041033163 で確認）。`alert-consecutive-failures` は `issues: write` を持つので、これも止める
- 依存関係を調べると、ワークスペースの 78 クレートのうち 23 個が `skippy-ffi`（パッチを当てた llama.cpp のビルド済みアーカイブが必要）に依存しており、素の ubuntu ではビルドできない。例: `mesh-llm`, `mesh-llm-host-runtime`, `skippy-runtime`, `skippy-serving`。残りの 55 個（`skippy-hf-hub` など）は CPU だけでビルドできる
- `cargo xtool repository affected-crates` の出力は `{affected, test_crates, all_rust, ...}`。`test_crates` は変更したファイルを持つクレートだけ。`all_rust=true`（`Cargo.lock` などを変更したとき。upstream を取り込むとたいていそうなる）のときは `test_crates` が空になり、`affected` にワークスペース全体が入る
- `.cargo/config.toml` は `rustc-wrapper = "sccache"` になっている。ホストランナーでは `RUSTC_WRAPPER=''` が必要（既存の `prepare-automation` の hosted-bare プロファイルと同じ）
- `.github/` 配下だけを変更しても、どのクレートにも対応しない（`test_crates` が空になる）。そのため、この PR 自身では clippy とテストが走らない
- xtask には、ワークフロー定義を検査する契約チェック（`tools/xtask/src/ci_validation/workflow_guards.rs`。action の SHA 固定、`run:` の中に `${{ }}` を書かない、など）がある。fork-ci.yml も既存の書き方に合わせ、`just ci-automation-contracts` などが通る形にする

## 変更内容

### 1. 新規: `.github/workflows/fork-ci.yml`

- **トリガー**: `pull_request`（opened, synchronize, reopened, ready_for_review, edited）と `push`（branches: [main]）
- **concurrency**: `fork-ci-${{ github.event.pull_request.number || github.ref }}`、cancel-in-progress
- **すべてのジョブ**に `if: github.repository == 'miminashi/mesh-llm'` を付ける。upstream を取り込んだときに upstream 側で動かないようにするため（`push` のジョブにも付ける）
- **共通 env**: `RUSTC_WRAPPER: ""`, `CARGO_BUILD_RUSTC_WRAPPER: ""`, `CARGO_INCREMENTAL: "0"`, `CARGO_PROFILE_DEV_DEBUG: "0"`, `CARGO_PROFILE_TEST_DEBUG: "0"`（キャッシュを小さくするため）, `MESH_LLM_SKIP_UI: "1"`
- action は既存と同じ SHA で固定する: `actions/checkout@fbc6f399…`（v5.1.0）、`dtolnay/rust-toolchain@4360b525…`、`actions/cache/restore` / `save@caa29612…`（v5.1.0）

ジョブ:

| ジョブ | 内容 |
|---|---|
| `commit_convention`（Commit convention、PR のみ） | `ci-quality-slice.yml` の同名ジョブと同じ手順。`./.github/actions/prepare-automation`（`runner-profile: hosted-bare`）で xtask を用意し、PR タイトルと各コミットのトレーラーを `repository conventional-commits` で検査する。PR 由来の文字列は env 経由で渡す |
| `rust_fmt`（Rust format） | rustfmt を入れて `cargo fmt --all -- --check` |
| `rust_checks`（Clippy and tests） | 下記 |

`rust_checks` の手順:

1. checkout（PR では `fetch-depth: 0`。base との差分を取るため）
2. toolchain（`components: clippy`）
3. `actions/cache/restore`: path は `~/.cargo/registry`, `~/.cargo/git`, `target`。key は `fork-ci-${{ runner.os }}-${{ toolchain cachekey }}-${{ hashFiles('Cargo.lock') }}`、restore-keys はその前方一致
4. `prepare-automation`（hosted-bare）で `$MESH_LLM_AUTOMATION_BIN` を用意する（キャッシュ済みの xtask を再利用する）
5. **変更ファイルを取る**: PR は `git diff --name-only "$BASE_SHA"...HEAD`。push は `"$BEFORE_SHA"..HEAD`（`before` がすべて 0 なら `Cargo.lock` を変更したことにして全体を対象にする）。SHA は env で渡す
6. **対象を決める**: `"$MESH_LLM_AUTOMATION_BIN" repository affected-crates --stdin` の結果から、`all_rust=true` なら `affected`、それ以外なら `test_crates` を使う。**変更ファイルに `.github/workflows/fork-ci.yml` が含まれるとき**は、CI 自身の動作確認として `skippy-hf-hub`（CPU だけでビルドできる）と `mesh-llm-host-runtime`（ネイティブが必要なので、除外の経路を確かめる）を追加する
7. **ネイティブが必要なクレートを除く**: 各クレートで `cargo tree -p "$crate" -e normal,build,dev --prefix none --format '{p}'` を実行し、依存に `skippy-ffi` があれば除く。除いたクレートは `$GITHUB_STEP_SUMMARY` に「skipped (needs native llama ABI)」として出す
8. 残ったクレートごとに `cargo clippy --locked -p "$crate" --all-targets -- -D warnings`
9. 残ったクレートごとに `cargo test --locked -p "$crate"`
10. `actions/cache/save`: **main への push のときだけ**保存する（PR は main のキャッシュを読むだけにして、キャッシュ容量の消費を抑える）

- timeout: `rust_checks` は 180 分（upstream を取り込んだときに、55 クレートすべてが対象になる場合を想定）
- 対象が 0 個のときは、clippy とテストを飛ばして成功にする（summary に「no Rust crates affected」と出す）

### 2. 既存ワークフローに条件を付ける（fork では動かさない）

条件は `github.repository == 'Mesh-LLM/mesh-llm'`。既存の `if:` があるジョブは `&&` でつなぎ、ないジョブには `if:` を 1 行足す。

| ファイル | 条件を付けるジョブ |
|---|---|
| `pr_linux.yml`, `pr_macos.yml`, `pr_windows.yml`, `pr_quality.yml`, `pr_website.yml` | `plan`（新しく `if:` を足す）、`required`（`!cancelled()` の前に足す） |
| `main_linux.yml`, `main_macos.yml`, `main_windows.yml`, `main_quality.yml`, `main_website.yml` | 同上 |
| `pr_ci_canary.yml` | `canary`（ラベルが付いたときだけ動くジョブ。複数行の `if: >-` の先頭に足す）。`inactive` は summary に書くだけなのでそのまま |
| `pr-cancel-sibling-runs.yml` | `monitor` |
| `website-pages.yml` | `build`（`deploy` は build に依存しているので skip される） |
| `windows-warm-caches.yml` | `warm_windows_cpu`, `warm_windows_gpu`, `prune_windows_abi_caches` |
| `stale-prs.yml` | `close_stale_prs` |
| `llama-upstream-canary.yml` | `result`, `alert-consecutive-failures`（`resolve` にはすでに条件がある） |

## 作業手順

1. ブランチ `ci/fork-only-ci` を切る
2. 上の 1・2 を実装する
3. ローカルで検査する（sccache が無いので `RUSTC_WRAPPER=` を付ける）
   - `actionlint`（入っていれば。無ければ `.github/actions/install-actionlint` と同じ版を scratchpad に入れる）
   - `just ci-automation-contracts` と `just ci-legacy-contracts`（ワークフローの契約チェック）。今回の変更で落ちる場合は、契約に合う書き方に直す
   - fork-ci の手順 5〜7 をローカルで試す（例: `printf 'skippy/crates/skippy-hf-hub/src/pagination.rs\n' | cargo xtool repository affected-crates --stdin`、`cargo tree -p mesh-llm-host-runtime …` に `skippy-ffi` が含まれること）
4. コミットする（Conventional Commits の形式。**`Co-Authored-By: Claude` トレーラーは付けない**。fork の Commit convention で弾かれるため）。push して PR を作る。PR 本文には、止めたワークフローの一覧と役割を issue #9 の表と対応づけて書き、`Closes #9` を付ける
5. PR の CI を確認する
   - `Fork CI` の 3 ジョブが success になること。`rust_checks` で `skippy-hf-hub` に clippy とテストが実行され、`mesh-llm-host-runtime` が skipped と出ること
   - PR · Linux / macOS / Windows / Quality / Website の各ワークフローのジョブが skipped になり、failure が出ないこと
6. レポートを `report/` に書き（CLAUDE.md のルールに従う。plan.md を添付する）、ユーザーに PR の URL と結果を伝える。マージはユーザーの判断に任せる（main へのマージ後に、main の push で Main · * が skipped になり、Fork CI がキャッシュを保存することを確かめる）

## 検証

- PR の Checks タブで、Fork CI の 3 ジョブが緑、upstream 由来のジョブはすべて skipped、failure が 0 であること
- `rust_checks` の summary に、テストしたクレートと skip したクレートが出ていること
- ローカルの契約チェック（`just ci-automation-contracts` / `just ci-legacy-contracts`）が通ること

## 残すもの（この PR ではやらない）

- ネイティブが必要な 23 クレート（`mesh-llm`, `mesh-llm-host-runtime` など）のテスト。必要になったら、`SKIPPY_LLAMA_AUTO_BUILD=1` などで llama.cpp を CPU 向けにビルドし、キャッシュする段階を足す
- ブランチ保護の必須チェックの設定（ユーザーが設定する。指定するなら Fork CI のジョブ名）
