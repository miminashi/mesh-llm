# HF pagination が別 origin の next URL に token を送る問題の修正（fork 向け PR）

## Context

miminashi/mesh-llm#2（jimoto-no-llm/mesh-llm#365 由来）の修正。`skippy/crates/skippy-hf-hub/src/pagination.rs` の `paginate` は、`Link: <...>; rel="next"` の URL を origin 照合なしで採用し、毎ページ `auth_headers()`（Bearer HF token）を付けて送る。応答を細工されると token が第三者 origin や `http://`（平文）へ漏れる。upstream には修正 PR が無い（#2579 は `get_file_metadata` のリダイレクトだけを same-origin に限定し、pagination は未対応）。ユーザー方針: upstream には出さず、fork `miminashi/mesh-llm` の main 向け PR として出し、マージして様子を見る。

## 方針

#2579 と同じ考え方（`Url::origin()` で scheme・host・port を比較）を pagination に適用する。別 origin の next は **エラーで打ち切る**（ヘッダを外して送るのではなく）。一覧 API が別 origin を next に返す正当な理由は無く、黙って送り続けるより失敗が見えるほうが安全。

## 変更内容（`skippy/crates/skippy-hf-hub/src/pagination.rs` のみ、現在 163 行）

1. `paginate` 冒頭で `initial_url.origin()` を保持する（`PaginationState` に `origin: url::Origin` を追加するか、クロージャで捕捉）。
2. `state.next_url = parse_link_header_next(...)` の直後で、`Some(next)` かつ `next.origin() != origin` なら
   `HFError::MalformedResponse { what: "pagination Link header points to another origin: {next}", url: Some(url.to_string()) }` を返す。
   - 既存の `error.rs` の `MalformedResponse`（成功ステータスだが応答の形が不正）を再利用し、新しい variant は足さない。
   - origin 比較は小さな関数 `same_origin_next(current_origin, next) -> Option<Url>` 相当に切り出し、単体テストしやすくする（`repository/listing.rs` の `same_host_redirect` と同じ書き方）。
3. `parse_link_header_next` 自体は変えない（`Url::parse` のままなので相対 URL や `\\host` はパース失敗で `None` = 終了、という現行動作を維持）。
4. コメントは周囲の密度に合わせ、「次ページへのリクエストも Hub token を運ぶので、別 origin には送らない」程度を 1〜2 行。

## テスト（同ファイルの `mod tests`）

- 単体: 起点 `https://huggingface.co/api/models` に対し、同一 origin の next は採用、`https://evil.example/x`・`http://huggingface.co/x`（格下げ）・`https://huggingface.co:8443/x`（別 port）・`https://cdn-lfs.hf.co/x` は拒否。
- 結合（`#[tokio::test]`）: `repository/listing.rs` のテストにある `TcpListener` による小さなローカル Hub の書き方（`serve` / `hub`）を真似て、
  - 1 ページ目が `Link: <http://localhost:PORT2/...>; rel="next"` を返し、別の listener（=別 origin）で待つ。`HFClientBuilder::new().endpoint(...).token("hf_dummy")` で一覧 API（例: `list_models` 系の公開 API、実装時に `repository/listing.rs` の paginate 呼び出し元から選ぶ）を回すと、エラーになり、別 origin 側に `Authorization` を含むリクエストが**届かない**ことを確認する。
  - 同一 origin の next は従来どおり 2 ページ目まで取れること。
  - 必要なら serve ヘルパは pagination 用に最小限を tests 内に書く（共有ヘルパ化はしない — AGENTS.md の「共有ヘルパを早まって作らない」に従う）。

## 作業手順

1. `git switch -c fix/hf-pagination-same-origin`（main から。未追跡の `CLAUDE.md` はコミットに含めない）
2. 上記の実装とテスト
3. 検証（Cargo は直列で実行）:
   - `cargo test -p skippy-hf-hub`（lib・結合・doc テストすべて）
   - `cargo clippy -p skippy-hf-hub --all-targets --no-deps -- -D warnings`
   - `cargo fmt --check`
   - `cargo xtool repository affected-crates skippy/crates/skippy-hf-hub/src/pagination.rs` で影響クレートを確認し、依存側（`skippy-model-hf` など）も `cargo test -p <crate>` を回す
4. コミット（`fix(skippy-hf-hub): keep the Hub token on pagination's own origin`、末尾に Co-Authored-By 行）
5. `git push origin fix/hf-pagination-same-origin`
6. `gh pr create -R miminashi/mesh-llm --base main --head fix/hf-pagination-same-origin`。本文は Summary / 変更点 / Testing / `Fixes #2`、末尾に Claude Code の署名行。upstream には出さない。
7. CLAUDE.md の規約どおりレポートを作成: `report/<JST timestamp>_hf_pagination_same_origin_fix.md`、`report/attachment/<名前>/plan.md` にこのプランをコピー、PR URL とテスト結果を記載（コード変更のみなので PNG は不要）。レポートは同じ PR ブランチにコミットするか、main 直コミットにするかは PR 作成時に既存の運用（`docs(report): ...` コミットが main にある）に合わせる。

## 検証の要点

- 新しい結合テストが、修正前のコードでは失敗（別 origin に token が届く）し、修正後は通ること（修正前に一度テストだけ当てて確認する）。
- 既存の pagination テスト（`test_parse_link_header_*`）と skippy-hf-hub の全テストが通ること。
