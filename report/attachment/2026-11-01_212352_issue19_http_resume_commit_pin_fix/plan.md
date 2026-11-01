# issue #19 修正プラン: HTTP cache 取得の GET を HEAD で確定した commit に固定する

## Context

`skippy-hf-hub` の cache 取得（`download_file_to_cache_network`、`skippy/crates/skippy-hf-hub/src/repository/download.rs`）では、HEAD も GET も、commit に解決する前の `revision`（例: `main`）の URL に送っている。HEAD と GET の間にブランチが更新されると、GET は新しいファイルを返す。それでも、古い ETag・古い commit の blob として確定してしまう。resume のときは、古い前半と新しい後半が混ざる（`abcd` + `UVWXYZ`）。partial ファイルが無いときは、新しい中身が丸ごと古い名前で保存される。

外部の人が出した PR #25 も同じ方針だが、ユーザーの判断で、独自に PR を作る（#25 は参考にとどめ、扱いはユーザーが決める）。

### 採らない案と理由
- **`If-Range`**: GET は HF からリダイレクトして LFS / xet の CDN に行く。CDN の ETag は HF の ETag（`X-Linked-Etag`）と別物なので、`If-Range` が常に不一致になり、毎回最初から取り直すことになる（resume が効かなくなる）
- **GET 応答の ETag・commit と HEAD の照合**: リダイレクト先の応答には `X-Repo-Commit` が付かないので、照合できない
- → **GET の URL を commit に固定する** のが素直で十分。commit に固定した URL の中身は変わらないので、resume も全体取得も HEAD と同じ版になる

## 変更内容

worktree: `/home/ubuntu/projects/mesh-llm-worktrees/fix-issue19-http-resume-identity`（ブランチ `fix/issue19-http-resume-identity`、`origin/main` から）

1. `download.rs` の `download_file_to_cache_network`:
   - `admit_commit` で検査した `commit_hash` から、既存の `self.hf_client.download_url(self.repo_type.url_prefix(), &repo_path, &commit_hash, &params.filename)` で GET 用の URL を作る
   - それを `http_download_response` に渡す。HEAD は今までどおり `revision` の URL に送る（ブランチの解決と `If-None-Match` のため）
   - `revision` がもともと commit なら、URL は今までと同じ
2. 回帰テストを、`download.rs` の既存の resume テスト群（`cache_download_resumes_http_incomplete_blob` など）と同じ module に足す。HEAD/GET の振る舞いを変えられる、ブランチが動く試験サーバを用意する:
   - HEAD `/resolve/main/...` → 古い commit と古い ETag、サイズ 10 を返す
   - GET `/resolve/main/...` → 新しい中身 `UVWXYZ...`（ブランチが動いた後の状態）を返す
   - GET `/resolve/<古い commit>/...` → 古い中身を返す（Range があれば 206）
   - テスト A（resume）: partial `abcd` から始め、結果が `abcdefghij` になり、GET の path に commit が入っていること
   - テスト B（partial なし）: 結果が古い中身の全体になること
   - 既存の resume 系テスト 3 本と、`content_range_matches` のテストは変えずに通ること
   - まず修正なしで A・B が失敗することを確かめてから修正する
3. 対象外: `download_file_to_local_dir`（mesh-llm 側に呼び出しが無い。HEAD がリダイレクトを追うので commit が取れず、別の設計が要る）は、PR とレポートで残課題として触れるだけにする

## 検証

CLAUDE.md のメモリ対策をした上で、ほかのセッションが重い cargo を動かしていないか `ps` で確かめてから実行する:

```bash
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER= CARGO_TARGET_DIR=/home/ubuntu/projects/mesh-llm/target
cargo test -j 8 --locked -p skippy-hf-hub
cargo fmt -p skippy-hf-hub -- --check
cargo clippy -j 8 --locked -p skippy-hf-hub --all-targets -- -D warnings
```

## コミット・PR・レポート

- コミット: `fix(skippy-hf-hub): pin cache GET to the commit resolved by HEAD`（本文に `Fixes #19`）。**Co-Authored-By トレーラーは付けない**（CI が拒否する。memory）
- レポート: `TZ=Asia/Tokyo date +%Y-%m-%d_%H%M%S` で取ったタイムスタンプで `report/<ts>_issue19_http_resume_commit_pin_fix.md` を作る。CLAUDE.md の書式（概要は平易な日本語、添付に plan.md）に従う。コード変更だけなので PNG は不要。別コミット `docs(report): ...` にする
- push して `gh pr create --repo miminashi/mesh-llm`。本文には問題、変更、If-Range を採らない理由、検証結果、PR #25 との関係（同じ方針で、resume なしのケースのテストを足したこと）、残課題（local_dir）、レポートへのパスを書き、末尾に Generated with Claude Code の行を付ける
- PR #25 には触らない（close もコメントもしない）
- ステータスラインのファイル `/home/ubuntu/.claude/statusline-session/8dd866db-ce6b-462e-860d-e434020895e7` を `issue #19 | PR #NN` に書き換える
- CI の結果を確認して報告する
