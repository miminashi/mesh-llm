# fork issue で指摘された脆弱性が手元コードで直っているかのレビュー

## Context

前回のレポート `report/2026-10-27_130344_fork_issue_security_triage.md` では、fork リポジトリ `jimoto-no-llm/mesh-llm` の issue から脆弱性に該当するものを抽出した。今回は、それらが **手元のコード**（upstream `Mesh-LLM/mesh-llm` の main `95f9b4810`、2026-10-22。現在のブランチ `docs/fork-issue-security-triage` はレポートを追加しただけでコードの差分はない）で直っているかを静的にレビューし、新しいレポートにまとめる。コードの修正はしない。

事前に確かめたこと:
- issue が参照しているコミット `18142efb0`（2026-10-16）は fork にしかなく、手元にはない。fork と upstream は分岐している（fork 側だけのコミットが 70、upstream 側だけのコミットが 154）。このため、fork で閉じた issue の修正が手元に入っていない可能性も、upstream 側で別に直っている可能性もある
- issue が参照しているファイルの代表例（`skippy-hf-hub/src/pagination.rs`、`tools/xtask/src/attestation.rs`、`just/mesh-client.just`、`payments/server.rs`、`plugin-manager/src/skills.rs` など）は手元にもある
- 前回レポートでは「該当 43 件」としたが、`triage_table.md` の行数は 42 件だった。今回は表の 42 件を対象にし、件数の食い違いを新しいレポートに注記する

## 対象（42 件）と分担

issue 本文は `report/attachment/2026-10-27_130344_fork_issue_security_triage/issues.json` から読む。

- **A. mesh crates（HTTP ingress・課金・plugin・identity など、17 件）**: #170, #76, #1, #2, #65, #68, #175, #266, #145, #176, #153, #192, #224, #269, #204, #264, #315
- **B. skippy crates（HF 取得・cache・protocol など、12 件）**: #365, #363, #359, #384, #361, #319, #358, #370, #373, #372, #371, #339
- **C. CI・scripts・just・docs（13 件）**: #474, #441, #475, #455, #459, #445, #451, #408, #270, #456, #461, #498, #541

## 手順

1. general-purpose agent を 3 つ並行で起動し、A/B/C を 1 つずつ担当させる（読み取り専用、編集はさせない）。各 issue について次を返させる:
   - 判定: `修正済み` / `一部修正` / `未修正` / `該当コードなし（構成が変わった）` / `判定不能`
   - 根拠: 手元コードの `path:line` と要点（issue が指摘した処理が今どうなっているか）
   - 修正済みの場合は、修正したコミットがあれば `git log -S` / `git log -L` で特定する
   - CLOSED の issue は、fork の修正 PR が手元に入っているかを確認する（`gh pr view -R jimoto-no-llm/mesh-llm` の diff と手元コードを比べる）
2. 統合したあと、優先度の高い件（#170, #365, #474, #441, #363, #359, #384, #224）と、`修正済み` と判定された件は、根拠の `path:line` を自分で読んで裏付ける
3. タイムスタンプを `TZ=Asia/Tokyo date +%Y-%m-%d_%H%M%S` で取得してレポートを書く

## 作成物

- レポート: `/home/ubuntu/projects/mesh-llm/report/<ts>_fork_issue_fix_status_review.md`
- 添付ファイル: `report/attachment/<ts>_fork_issue_fix_status_review/`
  - `plan.md`（このプランのコピー）
  - `fix_status_table.md`（42 件全部の判定と根拠 `path:line` の一覧）

## レポート構成（CLAUDE.md のカノニカル名）

- H1: `# fork issue の脆弱性が手元コードで直っているかのレビュー`（数値は入れない）
- メタ情報（実施日時・報告日時・作成者 Claude Opus 5.5）
- `## 概要`: 5〜8 段落の平易な日本語
- `## 添付ファイル`
- `## 核心発見サマリ`: 実測データではないので PNG は省略。`**結論**:` 段落と、深刻度 × 判定の件数表
- `## 前提・目的`: 前回レポートとの関係、fork と upstream の分岐、静的レビューであること
- `## 環境情報`: ブランチ・コミット、fork 側の基準コミット
- `## 参照レポート`: `./2026-10-27_130344_fork_issue_security_triage.md`
- `## 結果詳細`: High・Medium は 1 件ずつ根拠付き、Low はまとめた表
- `## 副次発見`: 件数の食い違い（43 → 42）、fork だけにある修正、issue の指摘とは別に見つかった問題など
- `## 残課題`: 実際の再現テストはしていない、未修正の件の修正方針、upstream への報告の検討

## 注意

- コードは編集しない。ビルドやテストも原則として実行しない（静的レビュー）
- token などの認証情報や、悪用手順の詳細はレポートに書かない
- commit はユーザーの指示があるまでしない

## Verification

- 42 件すべてに判定が付いていることを `fix_status_table.md` の行数で確認する
- 「修正済み」の根拠 `path:line` が実在するかを抜き取りで確認する
- レポート内の添付ファイルへのリンク先が存在することを確認する
