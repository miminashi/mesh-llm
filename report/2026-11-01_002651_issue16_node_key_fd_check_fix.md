# node 鍵の検査と読み取りを 1 つのファイル記述子で行う修正

- **実施日時**: 2026年11月1日 00:19 〜 02:09 JST (issue #16 の修正、回帰テストの追加、ローカル検証)
- **報告日時**: 2026年11月1日 02:09 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm は、ノードの身元を表す秘密鍵をファイルに保存しています。起動時にこの鍵を読むとき、これまでは「普通のファイルか確かめる」「権限を自分だけに直す」「中身を読む」の 3 つの手順を、それぞれファイル名から探し直して行っていました。そのため、確かめた直後に誰かがファイルを別の場所への symlink に差し替えると、別のファイルの権限を変えたり、攻撃者が用意した鍵を読み込んだりするおそれがありました（issue #16）。

起きるのは、鍵の置き場所を環境変数で、他のユーザーも書き込める既存のディレクトリに指定した場合に限られます。ふだんどおりホームディレクトリに置いているなら、他人は差し替えられません。

今回の修正では、鍵ファイルを最初に一度だけ開き、その開いたファイルに対して 3 つの手順をすべて行うようにしました。開くときに symlink をたどらない指定（`O_NOFOLLOW`）を付けているので、パスが symlink なら開く段階で拒否します。開いた後に名前を差し替えられても、手元で扱っているのは最初に確かめたファイルのままです。名前付きパイプ（FIFO）が置かれていても開く段階で止まらないよう、ブロックしない指定も付けました。

修正が効いていることを確かめるため、テストの中だけで働く仕掛けを入れました。鍵を開いた直後に、パスを偽の鍵への symlink に差し替える仕掛けです。修正前の処理に一時的に戻してこのテストを動かすと、偽の鍵を読んで失敗しました。修正後の処理では、本物の鍵を読み、偽の鍵の権限も変わりません。

既存の親ディレクトリの権限チェック（他人が書き込めるディレクトリなら警告やエラーにする）は、今回は入れていません。今までは権限 0755 の親ディレクトリも使えていて、それを確かめる既存テストもあるため、拒否すると今までの動作が変わってしまうからです。Windows の扱いも変えていません。

## 添付ファイル

- [実装プラン](attachment/2026-11-01_002651_issue16_node_key_fd_check_fix/plan.md)

## 前提・目的

- **背景**: issue #16（元は jimoto-no-llm/mesh-llm#153）。`ensure_private_node_key_file` が `symlink_metadata(path)` で確かめた後、`set_permissions(path)` と `read_to_string(path)` でパスを解決し直していた（TOCTOU）
- **目的**: 検査、chmod、読み取りを `O_NOFOLLOW` で開いた 1 つの descriptor に結び付ける。通常の鍵の読み取りと 0600 への修復は今までどおり
- **他所の状況（2026-11-01 確認）**: upstream の `node_key.rs` は同じ処理のままで、修正する PR は無い。jimoto-no-llm/mesh-llm#153 も open のまま

## 環境情報

- ワークステーション（Linux 6.8）、rustc stable。sccache と just が無いので `RUSTC_WRAPPER=` を付け、cargo を直接実行した
- ブランチ: `fix/issue16-node-key-fd-check`（`origin/main` = `6e83e2d0a` から）。修正コミット `f71857699`

## 結果詳細

### 変更内容

- `mesh/crates/mesh-llm-identity/Cargo.toml`: `[target.'cfg(unix)'.dependencies] libc = "0.2"` を追加した。使うのは定数（`O_NOFOLLOW` / `O_NONBLOCK` / `O_CLOEXEC` / `ELOOP`）だけで、crate の `#![forbid(unsafe_code)]` はそのまま。`Cargo.lock` には identity の依存に `libc` の 1 行が増える
- `mesh/crates/mesh-llm-identity/src/node_key.rs`:
  - `ensure_private_node_key_file(path) -> ()` を `open_private_node_key_file(path) -> File` に置き換えた。Unix では `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` で開く。`ELOOP`（symlink）は既存と同じ `InvalidKeyMaterial { "... is not a regular file" }` に変換する。そのあと `File::metadata`（fstat）で通常のファイルか確かめ、`mode & 0o077 != 0` なら `File::set_permissions`（fchmod）で 0600 に直す
  - `load_node_key_bytes_from_path` は、返ってきた `File` から読む
  - `save_node_key_bytes_to_path` の書き込み前後の検査も新しい関数を使う（戻り値は捨てる）
  - 非 Unix 版は、`symlink_metadata` で確かめた後に `File::open` する形にそろえた（動作は変わらない）

### 追加したテスト（`#[cfg(unix)]`）

| テスト | 確かめること |
|---|---|
| `path_swapped_after_open_does_not_redirect_chmod_or_read` | test 専用の hook で、開いた直後に鍵を偽の鍵（0644）への symlink に差し替える。本物の鍵を読み、偽の鍵は 0644 のまま、退避した本物の鍵は 0600 になる |
| `symlinked_node_key_is_rejected_and_target_untouched` | 鍵のパスが symlink なら `InvalidKeyMaterial` になり、リンク先の権限は変わらない |
| `fifo_node_key_is_rejected_without_blocking` | FIFO（`mkfifo` で作る）が置かれていても、止まらずにエラーになる |
| `loose_node_key_permissions_are_repaired` | 0644 の正しい鍵は読め、0600 に直る |

hook は `#[cfg(test)]` の thread-local で、本番のビルドには入らない。

修正前の、パスを解決し直す処理に一時的に戻すと（hook は `symlink_metadata` の直後に置く）、`path_swapped_after_open_does_not_redirect_chmod_or_read` が失敗した。読んだ鍵が偽の鍵の `[1; 32]` で、期待した `[7; 32]` と違っていた。なお、symlink と FIFO を拒否する 2 つのテストは、修正前でも通る（修正前も `symlink_metadata` で弾いていたため）。今後の変更で拒否が抜けないようにするためのテストである。

### ローカル検証

- `cargo test -p mesh-llm-identity --all-features`: 52 passed（node_key は 12 件）
- `cargo test -p mesh-llm-membership`: 55 passed
- `cargo test -j 4 -p mesh-llm-host-runtime`: lib は 4311 passed、1 failed、26 ignored。失敗は `mesh::gossip::tests::signed_records::bounded_exchange::oversized_snapshots_complete_real_quic_exchanges_in_both_directions` で、gossip 交換の 10 秒タイムアウト（`Elapsed`）。単独でも 3 回とも同じだった。lib が失敗したので、integration test は実行されていない。今回の修正とは関係ないと判断した（「副次発見」）
- `cargo clippy -p mesh-llm-identity --all-targets --all-features -- -D warnings`: 警告なし
- `cargo clippy -p mesh-llm --all-targets -- -D warnings`: 警告なし
- `cargo fmt --all --check`: OK
- `cargo xtool repository conventional-commits --range origin/main..HEAD`: OK
- `cargo xtool repository affected-crates`: `Cargo.lock` を含めると `all_rust: true`（全 crate が対象）になる。crate のパスだけで見ると、`test_crates` は `mesh-llm-identity` だけ
- 1 回目の `mesh-llm-host-runtime` のテストは、ワークステーションのメモリが足りなくなって Claude Code に止められた。ビルドの並列数を下げて再実行した

## 副次発見

- 上の gossip テストは、テスト用ノードを `Node::new_for_tests`（`SecretKey::generate()` で鍵をメモリ上に作る）で立てており、node 鍵のファイルを読み書きしない。fork の main の Linux CI（`f0e5ba1b5`）では通っている。このワークステーションで、ローカルの QUIC 通信が時間内に終わらないことによる失敗と見ている
- identity crate を修正前に戻して同じテストを動かす比較は、再ビルド中にメモリ不足で止められたので、結果は出ていない

## 残課題

- 既存の親ディレクトリが他人に書き込める状態（group/other の書き込み権限があり sticky bit が無い、所有者が自分でない、など）のときに警告またはエラーにするか。今の動作を変えることになるので、別 issue で判断する
- Windows の reparse point の扱い（今回は対象外）
- 同じ crate の `keystore.rs` / `ownership.rs` など、他の鍵ファイルの読み書きでも同じ形の問題が無いかは確かめていない
