# issue #20 の対応レポートを書く

## Context

issue #20（直接の release packaging が native runtime の checksum を照合せずに digest を付ける）について、次の作業をした。CLAUDE.md の「レポート執筆ルール」に従って、その記録を残す。

1. 噛み砕いた解説コメントを投稿した（17:34 JST）
2. `package-release.sh` を呼ぶ場所を調べ、この fork では配布用アーカイブを作らないので #3 と同じ扱いで閉じられると判断した
3. CLAUDE.md の注意事項に、`just bundle` / `just release-bundle` を使わない旨を 1 行足した
4. 閉じる理由のコメントを投稿し、not planned で close した（17:46 JST）

コードは変更していない。

## 手順

1. `TZ=Asia/Tokyo date +%Y-%m-%d_%H%M%S` でタイムスタンプを取る
2. `report/<ts>_issue20_direct_packaging_checksum_not_planned.md` を書く
3. `mkdir -p report/attachment/<ts>_issue20_direct_packaging_checksum_not_planned/`
   - このプランを `plan.md` としてコピーする
   - 投稿した 2 つのコメント本文を `gh issue view 20 --json comments` から書き出して添付する（`explanation-comment.md`、`close-comment.md`）

## レポートの構成

- H1: `# 直接 packaging の checksum 照合漏れ（issue #20）— fork では使わないので close`
- メタ情報: 実施日時 2026年10月31日 17:30 〜 17:47 JST（解説コメントの投稿、呼び出し元の調査、CLAUDE.md への追記、issue の close）、報告日時は手順 1 の時刻、作成者は Claude Opus 5.5
- `## 概要`: 平易な日本語で 5〜6 段落。何が問題か、解説を書いたこと、ユーザーから「この fork では組み立てないので #3 と同じく回避できないか」と提案があったこと、呼び出し元を調べて回避できると判断したこと、CLAUDE.md への追記と close、将来 bundle を作るときは開き直すこと
- `## 添付ファイル`: plan.md と 2 つのコメント
- `## 核心発見サマリ`: 実測データは無いので PNG は不要。`**結論**:` 段落に、呼び出し元が 3 か所であること、運用の経路が通らないこと、`NOT_PLANNED` で close したことを書く
- `## 前提・目的`
- `## 環境情報`: fork main `007d052d8`
- `## 結果詳細`: 問題の仕組み（precomposed と直接の経路の表、`package-release.sh:620-629`、`:495-502`、`compose.rs:98-99`）、呼び出し元の表（`release.yml` の 8 か所、`just/mesh.just:46-52` の `bundle`、`just/release-bundle.just`、`compute-changes` はパターンに名前があるだけ）、行った対応
- `## 参照レポート`: `./2026-10-31_023229_issue3_disable_release_workflows.md`、`./2026-10-31_172648_medium_security_issues_recheck.md`
- `## 残課題`: 将来 bundle を作るか release を戻すときは、#20 を開き直して `cp -R` の前に `verify-native-runtime-package.sh` を呼ぶ。CLAUDE.md は未追跡のままでコミットしていない

## 確認

- ファイル名、H1、メタ情報 3 行、`## 概要` が最初のセクションであることを目で確かめる
- 添付へのリンクが `attachment/...`（`./` なし）で、ファイルが実在することを `ls` で確かめる
- コミットはしない（依頼されていない）
