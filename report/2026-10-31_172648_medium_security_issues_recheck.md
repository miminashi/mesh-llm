# Medium の脆弱性課題の残存確認と起票

- **実施日時**: 2026年10月31日 17:13 〜 17:27 JST (トリアージで Medium とした 17 件を現行 main と照合し、残っている 13 件を起票)
- **報告日時**: 2026年10月31日 17:27 JST
- **作成者**: Claude Opus 5.5

## 概要

以前のトリアージでは、fork 元の issue の中から、Medium 相当の未解決の脆弱性を 17 件挙げていました。High の 5 件はすでにこの repo に起票して直してあるので、今回は Medium の各件がこの repo の現在のコードにまだ残っているかを確かめ、残っているものを High と同じ形式で起票しました。

確認では、3 つのサブエージェントで件を分けて、issue に書かれた箇所が今のコードのどこにあたるかを探してもらい、その結果を自分でもソースを読んで確かめました。fork 元の issue は少し前のコミットを指しているため、行番号ではなくコードの中身で照らし合わせています。

17 件のうち 13 件は、今もそのまま残っていました。just の recipe へのコマンド埋め込み、bucket sync や Skill のインストールでの指定ディレクトリ外への書き込み、ログ初期化での symlink 追従、node 鍵の検査と読み取りのずれ、metrics への credential の残存、サーバの申告どおりのメモリ確保、ダウンロード再開時の取り違え、release の checksum 検証漏れ、CI 周りの 3 件、無効にした plugin の起動です。これらを #12〜#24 として起票しました。

残りの 4 件は起票していません。HF の cache path と native runtime の path の 2 件は、upstream で検査が入り、直っていました。wan-lab の Docker の件は、wan-lab 自体が削除されていました。binary 受信の先読み上限の件は、1 frame の上限が 8MiB に下がり、問題の中心だった 512MiB の確保はできなくなっています。上限をわずかに超える余地は残りますが、コードのコメントにもそう書かれているので、起票は見送りました。

CI 関連の 1 件は一部だけ直っていたので、残っている部分だけを起票しました。CI 関連の 3 件と release の件は、この fork では関係する workflow や runner が動いていないため、今の時点で実際に影響があるのは限られます。

いずれもコードを読んで判断しただけで、実際に攻撃を再現したわけではありません。次は、起票した 13 件のうち影響の大きい、just の recipe（#12）とディレクトリ外への書き込み（#13・#14）から直すことを勧めます。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_172648_medium_security_issues_recheck/plan.md)
- [起票した issue 本文（fork の issue 番号ごと）](attachment/2026-10-31_172648_medium_security_issues_recheck/issue_bodies/)
- [fork の issue 番号と起票した issue URL の対応](attachment/2026-10-31_172648_medium_security_issues_recheck/created.tsv)

## 核心発見サマリ

**結論**: トリアージの Medium 17 件を `007d052d8` と照合した結果、残存 12 件、一部修正 2 件、修正済み 2 件、該当なし 1 件だった。残存の 12 件と、一部修正のうち #459 の残る部分の計 13 件を miminashi/mesh-llm #12〜#24 として起票した（タイトル `[security][Medium] …`、ラベル `bug`、本文は #2・#5 と同じ 概要／該当箇所／期待する動作／参考 の 4 節）。#319 は frame 上限 8MiB の導入で 512MiB の確保が解消済みのため、起票しなかった。

| fork # | 判定 | この repo の issue |
|---|---|---|
| #441 just recipe が引数を shell ソースへ補間 | 残存 | #12 |
| #363 HF の ETag・commit を cache path に使用 | 修正済み | — |
| #359 bucket sync がディレクトリの外に書き込む | 残存 | #13 |
| #384 runtime の id・version に path 検査なし | 修正済み | — |
| #224 `install_skills` が名前を検証しない | 残存 | #14 |
| #192 `.write_test` が symlink を辿る | 残存 | #15 |
| #153 node 鍵の TOCTOU | 残存 | #16 |
| #175 先頭空白 URL の credential が metrics に残る | 残存 | #17 |
| #361 size header のまま先行確保 | 残存 | #18 |
| #319 先読み上限 | 一部修正（実質解消） | 起票せず |
| #358 resume で HEAD と GET を照合しない | 残存 | #19 |
| #475 直接 packaging が checksum を検証しない | 残存 | #20 |
| #455 replay repair が検証器を保護しない | 残存（検証器は `tools/xtask` に移動） | #21 |
| #459 CI secret が job 全体の env にある | 一部修正 | #22（残る部分） |
| #445 wan-lab Docker が秘密ファイルを取り込む | 該当なし（wan-lab は削除） | — |
| #451 `EOF` 区切りが path と衝突 | 残存 | #23 |
| #269 無効化した plugin が CLI 経由で起動 | 残存 | #24 |

## 前提・目的

- **背景**: [前回のトリアージ](./2026-10-27_130344_fork_issue_security_triage.md)で jimoto-no-llm/mesh-llm の issue から脆弱性を洗い出した。High の 5 件は miminashi/mesh-llm の #1〜#5 で対応済み（すべて close）
- **目的**: Medium の未解決 17 件が現行コードに残っているかを確かめ、残っているものを起票する
- **前提**: 判定はコードを読んだだけで、攻撃の再現はしていない

## 環境情報

- 対象: miminashi/mesh-llm の `main` = `007d052d8`（origin/main と同じ。upstream/main をマージ済み）
- 起票先: miminashi/mesh-llm（#12〜#24）

## 結果詳細

### 修正済み・該当なしと判定した根拠

- **#363**: `skippy/crates/skippy-hf-hub/src/repository/commit_admission.rs` が追加された。commit は 40 桁の hex に限り、ETag は空・`.`・`..`・制御文字・`/` `\` `:` を拒否する。`download.rs:537-541` などで、path を作る前に必ず通している。公開 API の `get_file_metadata` は検査前の値を返すが、cache path には使われない
- **#384**: `skippy/crates/skippy-native-runtime/src/manifest.rs:287-301` の `validate_runtime_identity`（単一の `Component::Normal` に限る）と、`cache.rs:121-166` の `checked_runtime_dir`（canonical な cache root の内側かを確認する）が入った。install と remove はこれを通る。読み取りだけの `find_installed` は検査しない join のままだが、書き込みはしない
- **#445**: `skippy/evals/wan-lab/` は commit `496eec6a5`（#2461）で削除された
- **#319**: `message_receive.rs:221-229` が `MAX_STAGE_FRAME_BYTES`（8MiB、`skippy-protocol/src/validation.rs:33`）を frame の予算として渡し、宣言された長さで確保前に拒否するようになった。先に gate で止めてから後で加算する作りは変わらないため、最悪で約 32MiB＋1 frame を保持するが、`message_receive.rs:97-100` のコメントにそう明記されている。なお、L104 のコメントは 32MiB を「per-connection bound」と呼んだまま

### 一部修正（#459）

`smoke.yml` の pip（`.github/actions/setup-canary-python`）と npm の step は token を空にするようになった。`claude-live-model-gate.yml` の `ANTHROPIC_API_KEY` と、`ci-linux-product-smoke-slice.yml` の Laya smoke 4 job の `HF_TOKEN` は job の env に残っているので、そこだけを #22 に起票した。

## 副次発見

- `tools/oss-scanner/Dockerfile:29` は `COPY . /src` で repository 全体を build context から取り込み、`tools/oss-scanner/.dockerignore` も無い。root の `.dockerignore` は `.envrc`・`.env*`・`*.pem`・`*.key`・`.venv`・`.deps` を除外していないので、#445 と同じ種類の問題がある。手元で使うツールで、ビルドしたときにだけ問題になるため、起票はしていない
- #20（release）、#21・#22（self-hosted runner の workflow）、#23（CI）は、この fork では release を無効にしているか、runner が無いため、今の時点で実際に影響する場面は限られる

## 残課題

- 起票した 13 件の修正。優先度は #12（just のコマンド埋め込み）、#13・#14（ディレクトリ外への書き込み）を先にする
- 攻撃が実際に成立するかは試していない
- oss-scanner の Dockerfile を起票するかは、別に判断する
