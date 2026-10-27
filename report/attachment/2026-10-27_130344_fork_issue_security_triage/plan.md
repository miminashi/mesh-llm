# fork リポジトリ issue の脆弱性トリアージ結果をレポート化する

## Context

2026-10-23 のセッションで、fork リポジトリ `jimoto-no-llm/mesh-llm` の全 issue（474 件）を取得・精読し、脆弱性に該当するものを抽出した。さらに #441（just recipe のコマンドインジェクション）について、実際の recipe と CI の呼び出し経路を確認して深刻度を再評価した。この結果を、追加された `CLAUDE.md` のレポート執筆ルールに沿って `report/` に残す。

コード変更はなく、作成するのはレポート1本と添付ファイルのみ。

## 作成物

1. タイムスタンプ取得: `TZ=Asia/Tokyo date +%Y-%m-%d_%H%M%S`
2. レポート: `/home/ubuntu/projects/mesh-llm/report/<ts>_fork_issue_security_triage.md`
3. 添付ディレクトリ: `report/attachment/<ts>_fork_issue_security_triage/`
   - `plan.md`（このプランファイルのコピー。必須）
   - `issues.json`（取得した全 issue の生データ。scratchpad の issues.json からコピー）
   - `triage_table.md`（該当・グレーの全件一覧。レポート本文の表を短く保つため、詳細表はここに置く）

## レポート構成（CLAUDE.md のカノニカル名）

- **H1**: `# fork リポジトリ issue の脆弱性洗い出し`（数値はタイトルに入れず、核心発見サマリに書く）
- **メタ情報**
  - 実施日時: 2026年10月23日 00:51 〜 01:16 JST（全 issue の取得・分担精読・脆弱性判定、#441 の深刻度再評価）※セッションログのタイムスタンプから算出
  - 報告日時: 作成時に `date` で取得
  - 作成者: Claude Opus 5.5
- **## 概要**（5〜8 段落の平易な日本語。目的、やり方、主な発見、#441 の再評価、次の対応）
- **## 添付ファイル**（plan.md / issues.json / triage_table.md へのリンク）
- **## 核心発見サマリ**: 実測データではないので PNG は省略。`**結論**:` 段落と、件数の内訳（全 474 / 該当 42 / うち OPEN 37 / グレー約 50 / security ラベル 0）、最優先の issue（#170, #365, #474, path traversal 4 件、#441）
- **## 前提・目的**: 対象リポジトリ（Mesh-LLM/mesh-llm の fork）、判定基準（認可、情報漏えい、path traversal、インジェクション、DoS、整合性、TOCTOU・鍵管理）、除外基準
- **## 再現方法**: `gh issue list -R jimoto-no-llm/mesh-llm --state all --limit 2000 --json number,title,state,labels,body`、番号帯で 5 分割し並行精読したこと、#441 の確認に使ったファイル（`just/mesh-client.just`、`just/release-bundle.just`、`.github/workflows/release.yml`）
- **## 結果詳細**: High / Medium / Low の表（前回回答の内容を踏襲）、グレーのカテゴリ別要約
- **## 副次発見**: #441 の詳細（`{{ join }}` を bash ソースに埋め込む仕組み、CI は `scripts/package-release.sh "$RELEASE_TAG"` で直接呼ぶため経路外、招待 token の貼り付けを悪用するソーシャルエンジニアリング → 「ユーザーの操作が必要な任意コード実行」として High 寄り Medium に引き上げ）。security ラベルが一つも無いこと
- **## 残課題**: 深刻度は本文からの推定で実際の攻撃は検証していない、サブエージェント判定の一部は抜き取り確認（#170, #378）のみ、Upstream（Mesh-LLM/mesh-llm）にも同じ問題があるかは未確認、security ラベル付与や非公開報告（GitHub Security Advisory）への移行の検討

## 注意

- 認証情報・private-note の中身は書かない（#445 は「private-note.txt などが除外されていない」という事実のみ記載）
- 添付ディレクトリへのリンクには `./` を付けない
- `report/` は現在 untracked。commit はユーザーの指示があるまで行わない

## Verification

- `ls report/ report/attachment/<ts>_fork_issue_security_triage/` でファイルの存在を確認
- レポート内の相対リンク先がすべて存在するか `grep -o 'attachment/[^)]*' | xargs ls` で確認
- `## 概要` が H1 とメタ情報の直後にあることを目視で確認
