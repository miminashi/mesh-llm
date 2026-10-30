# HF 一覧 API のページ送りで token が別サーバへ渡る問題の修正

- **実施日時**: 2026年10月27日 22:10 〜 2026年10月30日 22:46 JST (pagination の origin 照合の実装とテスト、fork への PR 作成、CI 失敗の調査と PR の出し直し、CI 方針の issue 化、マージ)
- **報告日時**: 2026年10月30日 22:46 JST
- **作成者**: Claude Opus 5.5

## 概要

Hugging Face の一覧 API は、結果を複数ページに分けて返します。次のページの場所は応答ヘッダで知らせてきます。skippy の HF クライアントは、その「次のページ」の URL がどのサーバを指していても、同じ認証 token を付けてリクエストを送っていました。そのため、応答を細工されると、token が第三者のサーバに届いたり、暗号化されない http で送られたりします。fork の Issue でこの問題が指摘されていたので、修正しました。

upstream には、この問題を直す PR はまだありません。先日マージされた関連 PR は、ファイル情報を取得するときのリダイレクトに同じ対策を入れたもので、一覧のページ送りは対象外でした。この fork は upstream に PR を出すためのものではなく、自分たち用のセキュリティ修正を入れるためのものです。そこで修正は fork にだけ入れ、しばらく様子を見ることにしました。

修正の中身は次のとおりです。2 ページ目以降の URL が、最初のページと同じ scheme・host・port を持つときだけリクエストを送ります。別のサーバを指していたら、それまでに受け取った分を返したうえでエラーで止めます。テストは 3 つ追加しました。そのうち別サーバを指すケースのテストは、修正前のコードでは失敗し、修正後は通ることを確認しています。

PR を出したあと、CI が何度か落ちました。原因は 4 つで、どれも修正したコードとは関係のないものでした。

- upstream を取り込んだ main を fork に push し忘れていて、PR の差分に upstream の変更まで入っていた
- 古い main を基準に CI が走り、CI 設定の照合で失敗した
- このリポジトリの CI が、コミットに付いた AI エージェントの署名行を拒否した
- fork からは upstream の非公開 CI イメージを取得できない

最初の 3 つは、main の push、PR の開き直し、署名行を外した PR の出し直しで解消しました。最後の 1 つは fork の CI 環境の問題です。そのため、fork では upstream と同じ CI は動かさない方針を決め、issue にまとめました。

調査の途中で、fork の PR の CI は大部分が upstream の main にある CI 定義そのもので動いていることも分かりました。この仕組みと、fork の CI をどう組み直すかも、同じ issue に書いてあります。

最終的に、CI で落ちていたのが非公開イメージの件だけであること、最新の main にマージした状態でもテストが通ることを確認しました。そのうえで修正を fork の main にマージし、元の Issue は自動で閉じました。次は、実運用で一覧系の処理に問題が出ないかを見ることと、issue の方針に沿って fork 専用の CI を作ることです。

## 添付ファイル

- [実装プラン](attachment/2026-10-27_223410_hf_pagination_same_origin_fix/plan.md)
- [修正パッチ（当時の main との差分）](attachment/2026-10-27_223410_hf_pagination_same_origin_fix/fix.patch)

## 核心発見サマリ

**結論**: `HFClient::paginate` は、2 ページ目以降の URL の origin（`Url::origin()`）を最初の URL と比べるようになった。一致しない URL にはリクエストを送らず、`HFError::MalformedResponse` で打ち切る。

- 新しい結合テスト `a_next_page_on_another_origin_never_gets_the_token` は、修正前に FAILED、修正後に ok
- `cargo test -p skippy-hf-hub` は unit 210、integration 2、doc 21 がすべて pass。clippy（`-D warnings`）と fmt も問題なし
- マージ直前の fork の main（`c5064a8bf`）にマージした状態でも、同じ結果になった
- 修正は [miminashi/mesh-llm#7](https://github.com/miminashi/mesh-llm/pull/7) を merge commit `f81a43bc9` でマージし、#2 はクローズ済み
- fork の CI で落ちていたのは、非公開イメージ `ghcr.io/mesh-llm/mesh-llm-cuda-runner` を pull できないジョブと、その巻き添えのジョブだけだった。対応方針は [miminashi/mesh-llm#9](https://github.com/miminashi/mesh-llm/issues/9) にまとめた

## 前提・目的

- **背景**: [miminashi/mesh-llm#2](https://github.com/miminashi/mesh-llm/issues/2)（元は jimoto-no-llm/mesh-llm#365）。`pagination.rs` が Link ヘッダの `rel="next"` を origin の照合なしで採用し、毎ページ `auth_headers()` を付けて送っていた
- **upstream の状況**: Mesh-LLM/mesh-llm に該当する PR は無い（キーワード検索と、直近 200 PR の変更ファイルの走査で確認）。Mesh-LLM/mesh-llm#2579 は `get_file_metadata` のリダイレクトを same-origin に限定したが、`pagination.rs` は変更していない
- **目的**: fork の main 向けに修正 PR を出す。upstream には出さない（fork は自分たち用のセキュリティ修正のためのもの）

## 環境情報

- ワークステーション
- 修正コミット: `26585db14`（ブランチ `fix/hf-pagination-origin-check`、base は fork の main `9a248ca3c`）
- マージ: `f81a43bc9`（fork の main）
- Rust 1.99.1（作業中に `rustup update stable` で 1.90.0 から更新した）
- `.cargo/config.toml` は `rustc-wrapper = "sccache"` を指定しているが、ワークステーションには sccache が入っていない。そのため `RUSTC_WRAPPER=` を付けて実行した

## 結果詳細

### 変更（`skippy/crates/skippy-hf-hub/src/pagination.rs` のみ）

- `paginate` は `initial_url.origin()` を保持し、各ページで `next_url` を取り出すときに `same_origin_page(&origin, url)?` を通す
- `same_origin_page` は、origin が一致すれば `Ok(url)` を返す。一致しなければ `MalformedResponse { what: "pagination Link header points to another origin", url }` を返す。既存の error variant を使い、新しい variant は追加していない
- 比較はリクエストを送る直前に行う。そのため、別 origin を指していた場合でも、それまでのページの要素は呼び出し側に渡ってからエラーになる
- `parse_link_header_next` は変更していない

### テスト

| テスト | 内容 | 修正前 | 修正後 |
|---|---|---|---|
| `only_a_same_origin_next_page_is_requested` | 同じ origin は採用する。別ホスト、CDN、`http://` への格下げ、別 port は拒否する | （関数が無い） | ok |
| `a_same_origin_next_page_is_followed` | ローカル Hub で 2 ページ目まで読め、両方のリクエストに token が付く | ok | ok |
| `a_next_page_on_another_origin_never_gets_the_token` | 1 ページ目の Link が別 port の listener を指す。要素 1 件のあとエラーになり、別の listener にはリクエストが届かない | **FAILED** | ok |

- `cargo test -p skippy-hf-hub`: 210 + 2 + 21 passed（PR 作成時と、マージ直前の main にマージした状態の両方）
- `cargo clippy -p skippy-hf-hub --all-targets --no-deps -- -D warnings`: EXIT=0
- `cargo fmt --check -p skippy-hf-hub`: 問題なし
- `cargo xtool repository affected-crates`: `test_crates` は `skippy-hf-hub` だけ（公開 API は変えていない）

### PR と CI の経緯

| 順番 | 起きたこと | 原因 | 対応 |
|---|---|---|---|
| 1 | PR #6 の差分が 172 ファイルになった | upstream を取り込んだマージ `9a248ca3c` を fork に push していなかった | main を fast-forward で push し、`gh pr edit --base main` で PR の base を付け直した。差分は 1 ファイルになった |
| 2 | 全レーンが「source slice catalog differs from the protected planner catalog」で失敗した | CI が古い main（`4f7a91735`）を基準に走り、`ci/slices.yml` と `ci/ownership.yml` が PR 側と食い違った（upstream の #2537 の変更分） | PR を閉じて開き直し、CI を再実行させた |
| 3 | Commit convention が失敗した | このリポジトリは `Co-Authored-By: ... <noreply@anthropic.com>` などエージェントの署名行を拒否する | force push は権限で拒否されたので、署名行を外したコミットで PR #7 を出し直し、#6 を閉じた。Commit convention は pass した |
| 4 | Rust Clippy と Plan Linux が失敗した | Clippy は非公開イメージを pull できず `manifest unknown`。Plan Linux は Clippy が落ちたため「PR · Cancel sibling lanes」に止められた | コードとは無関係と判断。方針を issue #9 にまとめた |

マージ時点の #7 の CI は、pass 16、fail 7、skipping 8 だった。fail はすべて 4 の件とその集約ジョブ。main にブランチ保護は設定されていない。

## 副次発見

- **fork の PR の CI は upstream の定義で動く**: `pr_*.yml` は `uses: Mesh-LLM/mesh-llm/.github/workflows/ci-*-lane.yml@main` で lane を呼んでいる。その中の slice も、lane と同じ upstream のコミットのものが使われる。#7 の `PR · Linux` の run が参照したワークフローは、16 個すべて upstream の `3947de0cb` のものだった（API の `referenced_workflows` で確認）。そのため、fork の slice を直しても PR の CI は変わらず、upstream の main が変わると fork の PR の CI も変わる。一方、`main_*.yml` は相対パスで呼ぶので、fork 自身の定義で動く。詳細は issue #9 の補足に書いた
- **非公開イメージに依存するジョブ**: fork の main と PR で失敗したジョブ 15 個は、いずれも最初のエラーが `manifest unknown` だった。Clippy、Linux の Rust テスト、ネイティブランタイム、コンソール UI のビルド、Website などが該当する
- **fork で動いているスケジュール実行**: `stale-prs.yml`（Close stale pull requests）が fork でも動いて success になっている。fork の PR が自動で閉じられる可能性がある。`llama-upstream-canary.yml`（Llama Upgrade）は毎回 failure になっている
- **ワークステーションの Rust**: 1.90.0 では `xet-core-structures` がビルドできなかった（`floor_char_boundary` が unstable）

## 再現方法

```bash
git switch main   # f81a43bc9 以降
RUSTC_WRAPPER= cargo test -p skippy-hf-hub --lib pagination
```

## 参照レポート

- [fork の security Issue の修正状況レビュー](./2026-10-27_180227_fork_issue_fix_status_review.md)
- [fork の security Issue のトリアージ](./2026-10-27_130344_fork_issue_security_triage.md)

## 残課題

- 実運用で、HF の一覧系の処理（モデル解決、`list_tree` など）に問題が出ないかを見る。`HF_ENDPOINT` でミラーを使っていて、ミラーが別の origin を次のページとして返すと、`pagination Link header points to another origin` で止まるようになる
- issue #9 の方針に沿って、fork 専用の CI（Linux だけで clippy と Rust テストを実行する）を追加し、upstream から来たレーンを `if:` で止める
- リモートの作業ブランチ `fix/hf-pagination-origin-check` と `fix/hf-pagination-same-origin` を削除するかどうか
