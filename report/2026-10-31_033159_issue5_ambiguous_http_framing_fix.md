# OpenAI ingress のあいまいな HTTP framing を拒否する修正

- **実施日時**: 2026年10月31日 02:43 〜 13:49 JST (issue #5 の上流対応状況の再調査、issue への解説コメント投稿、jimoto-no-llm の PR #21 の移植とテスト、fork への PR 作成とマージ、aws-v100 への反映と実機確認)
- **報告日時**: 2026年10月31日 13:49 JST
- **作成者**: Claude Opus 5.5

## 概要

mesh-llm の OpenAI 互換 API は、HTTP リクエストの body の長さを示すヘッダが矛盾していても、黙って片方を選んで受け付けていました。前段にリバースプロキシがある構成では、プロキシと mesh-llm がリクエストの区切りを違う位置に読み、他人のリクエストが攻撃者のリクエストに混ざる「request smuggling」につながるおそれがあります。fork の issue #5 でこの問題を扱っていました。

まず、上流でこの問題が直っているかを改めて調べました。上流の最新 main でも該当箇所は変わっておらず、修正する PR も同じ内容の issue もありませんでした。名前の近い PR はいくつかありましたが、どれも別の問題を扱うものでした。

次に、issue を読む人向けに、仕組みと危険度を平易に解説したコメントを issue に投稿しました。body の長さを決めるヘッダの役割、区切りの解釈がずれると何が起きるか、mesh-llm の現状、どんな構成で危ないか、直し方を説明しています。

修正は、jimoto-no-llm の fork ですでにマージされている PR #21 を移植して行いました。矛盾したヘッダを持つリクエストは 400 で拒否し、後ろに転送するときは長さのヘッダを作り直します。あわせて、ヘッダと chunked の body が 1 回の読み込みで届き切った場合にサイズ上限を検査し損ねていた問題も、同じ PR で直りています。取り込みのときに、上流で後から入ったリアルタイム API 用の判定とぶつかった箇所が 1 か所あり、両方を残す形で解消しました。

テストは、変更したクレートについて単体・結合・ドキュメントテスト、clippy、fmt をすべて実行しました。単体テストで 1 件だけ失敗しましたが、単独で流し直すと修正前のコードでも修正後のコードでも通ったので、並列実行のときだけ落ちる不安定なテストで、今回の変更とは関係ないと判断しました。それ以外はすべて通っています。

修正は fork の main 向けの PR として出し、CI がすべて通ったのを確認してマージしました。元の issue は自動で閉じています。そのあと aws-v100 でビルドし直し、mesh-llm を新しいバイナリで起動し直しました。実機の API にあいまいなリクエストを送ると 400 で拒否され、正常なリクエストはこれまでどおり受け付けられることも確かめました。次は、ピアがつながった状態での実際の利用で問題が出ないかを見ることです。

## 添付ファイル

- [修正パッチ（fork の main との差分）](attachment/2026-10-31_033159_issue5_ambiguous_http_framing_fix/fix.patch)
- [issue #5 に投稿した解説コメント](attachment/2026-10-31_033159_issue5_ambiguous_http_framing_fix/issue5_comment.md)
- [PR #11 の本文](attachment/2026-10-31_033159_issue5_ambiguous_http_framing_fix/pr11_body.md)
- [テスト結果の要約](attachment/2026-10-31_033159_issue5_ambiguous_http_framing_fix/test_summary.txt)

plan mode は使っていないため、プランファイルはありません。

## 核心発見サマリ

**結論**: 上流（Mesh-LLM/mesh-llm main `61e2acfb9`）は未修正で、対応する PR も無い。jimoto-no-llm/mesh-llm#21 の 3 コミットを cherry-pick し、[miminashi/mesh-llm#11](https://github.com/miminashi/mesh-llm/pull/11)（Closes #5）を作成し、merge commit `6ef7e076b` でマージした（#5 は自動クローズ）。aws-v100 は `0.78.0+g6EF7E0` で稼働中

- 拒否するもの: `Content-Length` の重複（同じ値やカンマ区切りも含む）、TE と CL の併存、`chunked` 1 つだけではない TE（`chunked` が最後にない、ほかの coding がある、パラメータがある）。拒否すると `send_read_failure` → `send_400` で 400 を返す
- 転送時は、元の `Content-Length` / `Transfer-Encoding` を必ず捨て、`append_request_framing` で 1 つだけ付け直す
- ヘッダ上限: 1 回の読み込みで届き切ったヘッダ（`header_end > max_header_bytes`）と、作り直した転送用ヘッダも検査する
- chunked 上限: そのメッセージが実際に消費したバイト数で判定する
- 実機（aws-v100 の 9337）: CL 重複・TE+CL・`chunked, identity` はいずれも `400 Bad Request`（本文例 `{"error":{"message":"duplicate Content-Length",…}}`）、正常な CL リクエストはパースを通って 404（存在しないモデル名）
- テスト: `request_parse` 84 passed、lib 4211 passed / 1 failed（不安定なテスト。単独では main でも pass）、integration と doc はすべて pass、clippy・fmt も pass

## 前提・目的

- **背景**: [miminashi/mesh-llm#5](https://github.com/miminashi/mesh-llm/issues/5)（元は jimoto-no-llm/mesh-llm#1）。`request_parse.rs` が CL の重複を後勝ちで受理し、TE に `chunked` が含まれていれば CL を黙って捨て、body を書き換えない転送では元の framing ヘッダをそのまま送っていた
- **目的**: fork の main に修正を入れる。upstream には出さない（fork は自分たち用のセキュリティ修正のためのもの）

## 環境情報

- ワークステーション、Rust stable 1.99.1
- base: fork の main `c5470d0cb`（upstream main `61e2acfb9` をマージ済み）
- ブランチ `fix/issue5-ambiguous-framing`、HEAD `4439b52be`
- 取り込み元: jimoto-no-llm/mesh-llm#21（`2b5d5f33d`、`37e75ff1b`、`e0e076372`、2026-10-11 マージ。base は upstream の `43ddd24e6`）
- `.cargo/config.toml` は `rustc-wrapper = "sccache"` を指定しているが、ワークステーションに sccache が無いため `RUSTC_WRAPPER=` を付けて実行した。UI ビルドは `MESH_LLM_SKIP_UI=1` で省いた

## 結果詳細

### 上流の対応状況（2026-10-31 時点）

- `286bd1aea`（issue 作成時）から upstream main までに `request_parse.rs` に入ったのは #2560 と #2708（OCR のルーティング）だけで、framing の処理には触れていない
- 問題の箇所は upstream main の `request_parse.rs:827-839`（CL 上書き、TE の判定）と `:870-874`（TE+CL で CL を捨てる）に残っている
- 似た名前の PR
  - #2307「harden serving against hostile requests and peers」: マージ済みで、`286bd1aea` に含まれる
  - #2552「decode chunked bodies before re-framing…」: レスポンス側（`json_adaptation.rs`）の変更
- `request_parse.rs` を変更する open PR（#2710、#2703、#2654）の diff に、CL / TE / chunked の処理の変更は無い

### 取り込み

| コミット（このブランチ） | 元 | 内容 |
|---|---|---|
| `43ba4387a` | `2b5d5f33d` | `framing.rs` を追加。framing の判定を `request_framing` に集約し、転送時は framing を作り直す |
| `9fd3a99a6` | `37e75ff1b` | 届き切ったヘッダと chunked body にもサイズ上限を当てる。`read_budget_tests.rs` を追加 |
| `4439b52be` | `e0e076372` | 転送時に捨てられるヘッダで上限を超えるケースのテストを強化 |

- 1 つ目のコミットで `read_until_headers_parsed` がコンフリクトした。upstream 側で後から入った `upgrade_requested` / `connection_upgrade`（`/v1/realtime` の WebSocket upgrade 判定）のヘッダ読み取りと `is_realtime_upgrade` の計算を残し、CL / TE の読み取りと「TE があれば CL を捨てる」処理だけを `request_framing` に置き換えた
- `forwarded_request.rs` にも別の `finalize_forwarded_request` があるが、`request_parse` で作り直した後の `request.raw` を受け取るので、framing ヘッダは 1 つしか無く、変更は不要と判断した
- 変更は 3 ファイル、+562 / −32

### テスト

| コマンド | 結果 |
|---|---|
| `cargo test --locked -p mesh-llm-host-runtime --lib openai::request_parse` | 84 passed |
| `cargo test --locked -p mesh-llm-host-runtime --lib` | 4211 passed / 1 failed / 26 ignored |
| `cargo test --locked -p mesh-llm-host-runtime --test '*'` | すべて pass |
| `cargo test --locked -p mesh-llm-host-runtime --doc` | すべて pass |
| `cargo clippy --locked -p mesh-llm-host-runtime --all-targets --no-deps -- -D warnings` | pass |
| `cargo fmt --check -p mesh-llm-host-runtime` | pass |
| `cargo xtool repository affected-crates` | `test_crates` は `mesh-llm-host-runtime` のみ |

- lib テストで落ちた `runtime::tests::startup_models::layer_diagnostics::unresolved_layer_package_ref_surfaces_actionable_diagnostic`（"no active warnings surfaced for meshllm/regression-unavailable-layers"）は、モジュール単独で再実行すると、このブランチでも main でも 2 passed だった

## 再現方法

```bash
git fetch https://github.com/jimoto-no-llm/mesh-llm.git pull/21/head:refs/remotes/jimoto/pr21
git switch -c fix/issue5-ambiguous-framing main
git cherry-pick 2b5d5f33d 37e75ff1b e0e076372   # 1 つ目で request_parse.rs がコンフリクトする
export RUSTC_WRAPPER= MESH_LLM_SKIP_UI=1
cargo test --locked -p mesh-llm-host-runtime --lib openai::request_parse
```

## 参照レポート

- [fork の security issue のトリアージ](./2026-10-27_130344_fork_issue_security_triage.md)
- [fork の issue の修正状況の確認](./2026-10-27_180227_fork_issue_fix_status_review.md)
- [HF pagination の same-origin 修正（fork への修正 PR の前例）](./2026-10-27_223410_hf_pagination_same_origin_fix.md)

## マージと aws-v100 への反映

- PR #11 の CI: Clippy and tests（9 分 1 秒）、Commit convention、Rust format、PR / Linux・macOS・Windows・Quality・Website ほか 10 件が pass、11 件が skipping、失敗なし
- `gh pr merge 11 --merge` → `6ef7e076b`。ローカルの main も fast-forward した
- aws-v100: `ws-main` に push → `git merge --ff-only` → tmux `mesh-build` で `just build cuda 70`（`EXIT=0`、ビルド後のディスク空き 24G）→ `mesh` を停止・再起動
- `/api/status` の `version` は `0.78.0+g6EF7E0`、`node_status` は `Standby`（再起動直後でピア 0）
- 実機での確認（`nc` で 10.22.5.2:9337 に生の HTTP を送った）

| リクエスト | 応答 |
|---|---|
| `Content-Length` を 2 つ（値が違う） | `400 Bad Request` |
| `Content-Length` と `Transfer-Encoding: chunked` | `400 Bad Request` |
| `Transfer-Encoding: chunked, identity` | `400 Bad Request` |
| `Content-Length: 1` を 2 つ（同じ値） | `400`（`duplicate Content-Length`） |
| 正常な `Content-Length` 1 つ | `404 Not Found`（存在しないモデル名。パースは通った） |

## 残課題

- ピアがつながった状態で、split 推論が今までどおり動くかを見る
- 不安定な `layer_diagnostics` テストは今回の範囲外で、手を付けていない
