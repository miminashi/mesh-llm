# Medium 課題の残存確認と miminashi/mesh-llm への起票

## Context

トリアージレポート `report/2026-10-27_130344_fork_issue_security_triage.md` には、jimoto-no-llm/mesh-llm の issue から拾った Medium の未解決課題が 17 件挙がっている。High の 5 件はすでに miminashi/mesh-llm の #1〜#5 として起票済みで、どれも close されている。
今回は Medium の各件が現在の main（`007d052d8`、origin/main と同じ）にまだ残っているかを確かめ、残っているものを High と同じ形式で起票する。

## 確認結果（Explore エージェント 3 つで現行コードを照合した）

| fork # | 判定 | 起票 |
|---|---|---|
| #441 just recipe が引数を shell ソースへ補間する | 残存 | する |
| #363 HF の ETag / commit を cache path に使う | **修正済み**（`commit_admission.rs` で検査するようになった） | しない |
| #359 bucket sync がディレクトリの外に書き込む | 残存 | する |
| #384 runtime の id / version に path 検査がない | **修正済み**（`validate_runtime_identity`・`checked_runtime_dir` が入った） | しない |
| #224 `install_skills` が skill 名を検証しない | 残存 | する |
| #192 `.write_test` が symlink を辿る | 残存 | する |
| #153 node 鍵の TOCTOU | 残存 | する |
| #175 先頭空白のある URL の credential が metrics に残る | 残存 | する |
| #361 size header の値でそのまま先行確保する | 残存 | する |
| #319 lookahead の予算 | **一部修正**（frame 上限が 8MiB になった。gate で止めてから後で加算するため、最大で約 32MiB＋1 frame まで保持し、コメントとも食い違う） | 残る部分だけを起票する |
| #358 resume で HEAD と GET の identity を照合しない | 残存 | する |
| #475 直接 packaging が runtime の checksum を検証しない | 残存（compose は Rust の xtask に移った） | する |
| #455 replay repair が検証器を保護しない | 残存（検証器は `tools/xtask` に移った。保護対象は依然 root 直下だけ） | する |
| #459 CI secret が job 全体の env にある | **一部修正**（pip と npm は直った。Claude live gate の API key と Laya GPU job の HF_TOKEN は job env に残る） | 残る部分だけを起票する |
| #445 wan-lab の Docker が秘密ファイルを取り込む | **該当なし**（wan-lab は削除された） | しない（`tools/oss-scanner/Dockerfile` の `COPY . /src` に似た問題がある。これはレポートに副次発見として書き、起票はしない） |
| #451 `EOF` 区切りが path と衝突する | 残存 | する |
| #269 無効化した plugin が CLI 経由で起動できる | 残存 | する |

→ 起票するのは **14 件**（残存 12 件と、一部修正のうち残る部分 2 件）。

## 起票の形式（#2・#5 に合わせる）

- タイトル: `[security][Medium] <平易な日本語で一文>`
- ラベル: `bug`
- 本文:
  ```
  ## 概要
  （2〜3 文。何が起きるか、どういう条件で問題になるか）

  ## 該当箇所（`007d052d8` 時点）

  - [`<crate>/src/<file>.rs:L1-L2`](https://github.com/miminashi/mesh-llm/blob/007d052d8/<full path>#L1-L2): 説明
  ...

  ## 期待する動作
  （fork の issue の「対応内容」「完了条件」を要約する）

  ## 参考

  - fork の issue: jimoto-no-llm/mesh-llm#NNN（fork 側での確認内容を一言）
  - この repo では、コードを読んで判定しただけです（一部修正のものは、直った部分も書く）
  ```
- 行番号はエージェントの報告をもとに、起票前に私が該当行を `sed -n` で読んで確かめる（各件の主要な箇所）

## 手順

1. 各件の該当行を読み直し、行番号と判定を確定する（読むだけ）
2. 本文を scratchpad に 1 件 1 ファイルで書き、`gh issue create --repo miminashi/mesh-llm --label bug --title ... --body-file ...` で 14 件を起票する。順序はトリアージ表の並び（#441 → #269）に合わせる
3. `gh issue list --repo miminashi/mesh-llm` で 14 件ができたことと、リンク先の行が合っていることを抜き取りで確かめる
4. レポートを作る: `report/<TZ=Asia/Tokyo date>_medium_security_issues_recheck.md`（CLAUDE.md の形式に従う。実測データは無いので PNG は不要）。確認結果の表、起票した issue 番号との対応、修正済み・該当なしになった件の根拠、oss-scanner の副次発見を書き、このプランを `plan.md` として添付する。コミットはしない

## 検証

- 起票後に `gh issue view <n> --repo miminashi/mesh-llm` で 2〜3 件の表示を確かめ、リンクを開いて行がずれていないことを確かめる
