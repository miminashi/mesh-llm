# ログ初期化の書き込み確認が既存ファイルを壊す問題の修正

- **実施日時**: 2026年11月1日 00:09 〜 17:52 JST (issue #15 の解説コメント、upstream と jimoto-no-llm の対応状況の確認、修正とテスト追加、検証、PR 作成。途中、メモリ不足によるテストの中断とリンカーの切り替えをはさむ)
- **報告日時**: 2026年11月1日 17:52 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm は起動時に、ログを保存するフォルダを用意し、そこに本当に書き込めるかを確かめます。これまでは、決まった名前のファイルを書いてすぐ消す、というやり方でした。この書き方だと、同じ名前のファイルが先にあれば中身を空にして消してしまいます。それが別の場所を指す symlink なら、指している先のファイルを空にしてしまいます。issue #15 はこの問題です。ログフォルダを他人も書き込める場所にしている場合には、他人が仕込んだ symlink 経由で、自分のファイルを壊されるおそれがありました。

作業の前半では、issue に噛み砕いた解説をコメントし、upstream と jimoto-no-llm/mesh-llm に修正の提案がないかを確かめました。どちらにも見当たりませんでした。

修正では、確かめ方を「名前がほかと重ならない新しいファイルを作る。同じ名前のものがあれば、開かずに失敗する」に変えました。消すのも自分で作ったファイルだけです。既にある同名のファイルや symlink、その指す先には一切触れません。書き込めるフォルダはこれまでどおり使え、書き込めないフォルダはこれまでどおり「ログ無効」で起動を続けます。

既にあるファイルが壊れないこと、symlink の先が壊れないこと、読み取り専用のフォルダは失敗と判定されることをテストに加えました。修正前のコードでは、既存ファイルと symlink の 2 件が失敗し、問題を再現できることも確かめました。

検証の途中で、ワークステーションのメモリが足りなくなり、テストが 2 回止められました。原因は、テスト用プログラムを組み立てるリンカーが 1 本あたり数 GB のメモリを使い、それが並列で走ったことです。rustup に同梱されている軽いリンカー（lld）に切り替えて解決しました。この手順は CLAUDE.md に追記し（別の PR）、ほかの作業セッションにも伝えました。

crate 全体のテストでは、修正と関係のないテストが 2 件失敗しました。1 件は全体を並列に流したときだけタイミングで落ちるもので、単独なら通ります。もう 1 件は修正前のコードでも同じように落ちる、もともとの失敗です。このレポートは修正と同じ PR に含めています。マージされたら worktree を片付けます。

## 添付ファイル

- [crate 全体の lib テスト結果（抜粋）](attachment/2026-11-01_175159_issue15_logging_write_probe_fix/cargo-test-host-runtime-lib.log)
- [統合テストと doc テストの結果](attachment/2026-11-01_175159_issue15_logging_write_probe_fix/cargo-test-host-runtime-integration-doc.log)
- [GNU ld でリンクしたときの空きメモリ推移](attachment/2026-11-01_175159_issue15_logging_write_probe_fix/memory-gnu-ld.log)
- [rust-lld でリンクしたときの空きメモリ推移](attachment/2026-11-01_175159_issue15_logging_write_probe_fix/memory-rust-lld.log)

plan mode は使っていないので、実装プランはありません。

## 核心発見サマリ

**結論**: `try_create_dirs` の書き込み確認を、`root.join(".write_test")` への `std::fs::write` と `remove_file` から、新しく追加した `root_accepts_new_files` に置き換えた。これは `tempfile::Builder::new().prefix(".write_test-").tempfile_in(root)` で作る（`O_EXCL` でランダム名）。成功したら `close()` で自分の作ったファイルだけを消す。`tempfile` は既存の依存なので、`Cargo.toml` は変えていない。追加テスト 3 件を含め、`logging::foundation` は 15 件すべて通過した。修正前のコードでは、既存ファイルと symlink の 2 件が失敗する。`cargo test -p mesh-llm-host-runtime` は lib が 4313 passed / 2 failed / 26 ignored、統合テスト 6 本と doc テストはすべて通過した。失敗した 2 件はどちらも修正とは無関係だった。`accepted_delete_timeout_returns_pending_without_a_failure_audit` は単独なら 3/3 で通る。`oversized_snapshots_complete_real_quic_exchanges_in_both_directions` は修正前の 6e83e2d0a でも同じ箇所（`bounded_exchange.rs:79` の `Elapsed`）で失敗する。fmt・clippy（`mesh-llm-host-runtime` と `mesh-llm`）・`cargo check -p mesh-llm`・`no-console-print` はすべて通過した。

## 前提・目的

- **背景**: issue #15（元は jimoto-no-llm/mesh-llm#192）。`mesh/crates/mesh-llm-host-runtime/src/logging/foundation.rs` の `try_create_dirs` が、固定名 `.write_test` に `fs::write` してから `remove_file` していた。既存の同名ファイルは中身を消されたうえで削除され、Unix の symlink は参照先が空にされた
- **目的**: 書き込み確認で既存のエントリと symlink の参照先に一切触れないようにする。書き込めるかどうかの判定結果は変えない
- **前提条件**: ログの root は、設定の `application_state_root`、`MESH_LLM_DATA_DIR/logging`、`~/.mesh-llm/logging` の順に決まる。ホームが取れない環境では `/tmp/mesh-llm/logging` になる

## 環境情報

- ワークステーション（32 コア、メモリ 31GB、swap 4GB）。aws-v100 は使っていない
- worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue15-logging-write-test`（ブランチ `fix/issue15-logging-write-test`、base `6e83e2d0a`）
- コミット: `897bde393 fix(logging): probe the logging root with a fresh exclusive file`
- 変更ファイル: `mesh/crates/mesh-llm-host-runtime/src/logging/foundation.rs` のみ（+86 / −6）
- Rust stable 1.99.1。リンカーは rustup 同梱の rust-lld（LLD 23.1.1）

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-issue15-logging-write-test
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -j 8 -p mesh-llm-host-runtime --lib logging::foundation
cargo test -j 8 -p mesh-llm-host-runtime
cargo fmt --all --check
cargo clippy -p mesh-llm-host-runtime --all-targets -- -D warnings
cargo check -p mesh-llm
cargo clippy -p mesh-llm --all-targets -- -D warnings
cargo run -q -p xtask -- repo-consistency no-console-print
cargo xtool repository affected-crates mesh/crates/mesh-llm-host-runtime/src/logging/foundation.rs
```

修正前のコードでの確認では、`root_accepts_new_files` の本体を一時的に元の `fs::write` + `remove_file` に戻して `logging::foundation` のテストを流し、そのあと修正版に戻した。

## 結果詳細

### 追加したテスト

| テスト | 内容 | 修正前 | 修正後 |
|---|---|---|---|
| `init_leaves_existing_write_test_file_untouched` | 既存の `.write_test`（中身 `keep me`）が残り、中身も変わらない。`.write_test-*` が残らない | FAILED | ok |
| `init_does_not_write_through_existing_write_test_symlink`（Unix） | `.write_test` → root の外のファイル、という symlink の参照先の中身が変わらず、symlink も残る | FAILED | ok |
| `init_read_only_root_fails_open`（Unix） | 0o555 の root は unhealthy になる。権限ビットが効かない特権ユーザーでは判定をスキップする | ok | ok |

### 検証コマンド

| コマンド | 結果 |
|---|---|
| `cargo test -p mesh-llm-host-runtime --lib logging::foundation` | 15 passed |
| `cargo test -p mesh-llm-host-runtime`（lib） | 4313 passed / 2 failed / 26 ignored（2 件は下記のとおり修正と無関係） |
| `cargo test -p mesh-llm-host-runtime --test '*'` / `--doc` | 統合テスト 6 本すべて ok、doc テストは 0 件で ok |
| `cargo fmt --all --check` | ok |
| `cargo clippy -p mesh-llm-host-runtime --all-targets -- -D warnings` | ok |
| `cargo check -p mesh-llm` / `cargo clippy -p mesh-llm --all-targets -- -D warnings` | ok |
| `no-console-print` | ok |
| `affected-crates` | `test_crates` は `mesh-llm-host-runtime` のみ |

### 修正と無関係な失敗

- `api::routes::logs::delete::tests::accepted_delete_timeout_returns_pending_without_a_failure_audit`: 全体実行では `accepted timeout response: MaintenanceCancelled` で失敗した。単独では 3 回とも通る。プロセス全体で共有するログの runtime 状態と 1 秒のタイムアウトに依存しており、並列実行の負荷で落ちたと考えられる
- `mesh::gossip::tests::signed_records::bounded_exchange::oversized_snapshots_complete_real_quic_exchanges_in_both_directions`: 単独でも 3 回とも `Elapsed` で失敗した。修正を stash した base（`6e83e2d0a`）でも同じように失敗するので、もともとの失敗

## 副次発見

- **リンク時のメモリ不足**: ワークステーションには mold も lld も無く、`scripts/cargo-linker` は GNU `ld` に fallback する。`cargo test -j 8` では GNU `ld` が 5 本並び、1 本あたり約 5GB を使った。空きメモリは約 1.4GB まで落ち（swap も使用）、Claude Code がバックグラウンドのコマンドを 2 回止めた。rustup 同梱の `gcc-ld/ld.lld` を PATH の先頭に足すと、probe が lld を選ぶ。そうすると同じテストでも、空きメモリの最低値は約 19.5GB で済んだ
- lld の RSS は ps では 1 本あたり約 5.5GB に見えるが、`RssAnon` は約 1GB で、残り約 4.5GB は mmap した入力ファイル（`RssFile`）だった
- この手順を CLAUDE.md に追記する PR #37 を作り、ほかの mesh-llm セッション 3 つにも伝えた（fix-http-url-credential-redaction への送信は失敗）

## 残課題

- PR マージ後に worktree `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue15-logging-write-test` を片付ける
- `oversized_snapshots_complete_real_quic_exchanges_in_both_directions` はワークステーションで常に失敗する。CI でも落ちるかは、この PR の CI で確かめる
