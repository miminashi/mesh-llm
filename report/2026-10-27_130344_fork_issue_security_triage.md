# fork リポジトリ issue の脆弱性洗い出し

- **実施日時**: 2026年10月23日 00:51 〜 01:16 JST (fork リポジトリの全 issue を取得・精読して脆弱性を判定し、#441 の深刻度を再評価)
- **報告日時**: 2026年10月27日 13:03 JST
- **作成者**: Claude Opus 5.5

## 概要

fork リポジトリ（jimoto-no-llm/mesh-llm）には品質改善の issue が大量に起票されていますが、セキュリティ上の問題を示すラベルは一つも付いていません。そこで全 issue を読み、脆弱性として扱うべきものがどれだけ紛れているかを洗い出しました。

全 issue を本文ごと取得し、番号帯で 5 つに分けて並行して読みました。判定には、認可の欠如、秘密情報の漏えい、指定ディレクトリ外へのファイル操作、インジェクション、外部入力によるサービス停止、署名や checksum の検証漏れ、鍵管理の不備という共通の基準を使いました。単なる機能バグや表示、テスト品質、ドキュメントの話は除外し、判断が分かれるものは「グレー」として別に分けました。

その結果、全体の約 1 割が脆弱性に該当し、そのほとんどは未解決のままでした。特に影響が大きいのは、有料 model を支払いなしで使われる恐れがある課金判定の不備、Hugging Face の token が別のサーバに送られてしまう問題、署名が無効な配布物でも release 工程を通ってしまう問題の 3 つです。

次に多かったのは、外部サーバの応答やファイル名を信用してそのままパスに使い、指定した場所の外にファイルを書いてしまう種類の問題で、Hugging Face 取得、bucket 同期、runtime の取り込み、Skill のインストールなど複数の箇所にあります。ほかにも、CI の秘密情報が必要以上の手順に渡っている、依存パッケージのバージョンが固定されていない、といったサプライチェーン寄りの問題がまとまって見つかりました。

ユーザーから深刻ではないかと指摘された #441 は、実際の just recipe と CI の呼び出し方を確認して評価し直しました。細工した招待 token を開発者が `just` に貼り付けて実行すると、その人の権限で任意のコマンドが動きます。外から自動で侵入される経路ではありませんが、招待 token はもともと他人から受け取って貼り付けるものなので悪用しやすく、当初の評価より一段高く扱うべきだと判断しました。一方、CI や release workflow はこの recipe を経由しないため影響を受けません。

なお、深刻度はすべて issue 本文の記述から推定したもので、実際に攻撃を試したわけではありません。また、各番号帯の判定は分担して読んだ結果で、全件を一人で再確認したわけではなく、一部を抜き取って確認したにとどまります。

次の対応としては、未解決で影響の大きい 3 件と、ディレクトリ外への書込み 4 件、それに #441 を優先して直すことを勧めます。あわせて、該当 issue にセキュリティ用のラベルを付けて区別することと、fork 元のリポジトリにも同じ問題があるかを確かめ、必要なら非公開の経路で報告することを検討してください。

## 添付ファイル

- [実装プラン](attachment/2026-10-27_130344_fork_issue_security_triage/plan.md)
- [該当・グレーの全件一覧](attachment/2026-10-27_130344_fork_issue_security_triage/triage_table.md)
- [取得した全 issue の生データ（JSON）](attachment/2026-10-27_130344_fork_issue_security_triage/issues.json)

## 核心発見サマリ

**結論**: 全 474 件（OPEN 446 / CLOSED 28）のうち、脆弱性に該当するのは 43 件（OPEN 37 / CLOSED 6）、グレーは 64 件だった。`security` 系のラベルが付いた issue は 0 件で、該当 issue は品質改善 issue に埋もれている。未解決のうち最優先は #170（課金ゲートの迂回、P1）、#365（HF token の別 origin 送信）、#474（attestation 無効でも release を通る）の 3 件。続いて path traversal の #363・#359・#384・#224 と、ユーザー操作を条件とする任意コード実行の #441。

| 項目 | 件数 |
|---|---|
| 全 issue | 474（OPEN 446 / CLOSED 28） |
| 脆弱性に該当 | 43（OPEN 37 / CLOSED 6） |
| 　High | 5（うち OPEN 3） |
| 　Medium | 18（#441 を含む、うち OPEN 17） |
| 　Low〜Medium / Low | 20（うち OPEN 17） |
| グレー | 64 |
| `security` ラベル付き | 0 |

※ 2026-10-23 の会話での回答では「該当 42 件」としていた。一覧表に整理し直して数え直した結果、43 件が正しい。

## 前提・目的

- **背景**: fork リポジトリには 2026-10-16 前後に品質改善の issue が大量に起票されている。ラベルは `bug` / `enhancement` / `area:*` / `priority:*` のみで、セキュリティの区分がない
- **目的**: 全 issue から脆弱性に該当するものを抽出し、深刻度の目安を付けて優先順位を示す
- **対象**: `jimoto-no-llm/mesh-llm`（`Mesh-LLM/mesh-llm` の fork）の OPEN・CLOSED を含む全 issue
- **判定基準**: 認証・認可の欠如やなりすまし、機密情報の漏えい、path traversal / symlink 追従、コマンド・ヘッダ・HTTP framing などのインジェクション、外部入力起点の DoS、checksum・署名・attestation の検証漏れや CI secret の過剰付与、TOCTOU・鍵管理の不備
- **除外基準**: 機能バグ、表示崩れ、テスト品質、ドキュメント修正。子 issue の一覧だけの tracking issue（#109〜#137）

## 再現方法

1. 全 issue を本文付きで取得する

   ```bash
   gh issue list -R jimoto-no-llm/mesh-llm --state all --limit 2000 \
     --json number,title,state,labels,body > issues.json
   ```

2. 番号帯（#1〜150、#151〜250、#251〜340、#341〜440、#441〜543）で Markdown に分割し、それぞれを上記の判定基準で精読する。5 つのサブエージェントに並行して読ませ、結果を統合した
3. 判定の抜き取り確認として、#170（該当）と #378（除外）の本文を直接読んで判定が妥当なことを確かめた
4. #441 は次のファイルを読んで評価し直した
   - `just/mesh-client.just`（`{{ join }}` などを bash ソースに埋め込んでいる）
   - `just/release-bundle.just`
   - `.github/workflows/release.yml`（`scripts/package-release.sh "$RELEASE_TAG" dist` を直接呼んでいる）

## 結果詳細

全件の一覧は [triage_table.md](attachment/2026-10-27_130344_fork_issue_security_triage/triage_table.md) にある。ここでは High と Medium の未解決分を挙げる。

### High

| # | 状態 | 内容 | 分類 |
|---|---|---|---|
| #170 | OPEN | 公開 model ID を内部名に書き換えた後で価格を照合するため、remote peer が有料 model を支払いなしで使える可能性がある（P1） | 課金ゲートの迂回 |
| #365 | OPEN | HF 一覧の pagination で、別 origin の `next` Link にも HF の Bearer token を送る | token 漏えい |
| #474 | OPEN | attestation が無効・欠落でも検査が終了コード 0 を返し、composer がそれを通す | 署名検証の迂回 |
| #76 | CLOSED | Stage Stop 要求の送り主が現 coordinator か確認していなかった | 認可の欠如 |
| #1 | CLOSED | Content-Length の重複や TE と CL の併存を受理して上流へ送っていた | HTTP request smuggling |

### Medium（未解決分）

| # | 内容 | 分類 |
|---|---|---|
| #441 | just recipe が引数を bash ソースへ補間し、細工した join token でコマンドが動く | コマンドインジェクション |
| #363 | HF の ETag・X-Repo-Commit を検証せず cache path に join する | path traversal |
| #359 | bucket sync が remote の path を検証せずに書き込む | path traversal |
| #384 | runtime manifest の id・version を検証せず、cache 外へコピーされ得る | path traversal |
| #224 | `install_skills` が skill 名を検証せず、root の外に書き込める | path traversal |
| #192 | ログ初期化の `.write_test` が symlink を辿り、参照先を空にする | symlink 追従 |
| #153 | node 鍵を検査した後に開き直すため、鍵をすり替えられる | TOCTOU / 鍵管理 |
| #175 | 先頭空白のある endpoint URL の credential が metrics に残る | 情報漏えい |
| #361 | server が申告したサイズのまま先にメモリを確保する | DoS |
| #319 | binary stage の受信が 32MiB 上限を超えて 512MiB まで確保する | DoS |
| #358 | resume で HEAD と GET の identity を照合せず、新旧の内容が混ざる | 整合性 |
| #475 | 直接 packaging が既存 checksum と照合せずに digest を付け直す | 整合性 |
| #455 | 修復 agent が検証スクリプト自体を書き換えられる | CI の整合性 |
| #459 | CI の API key と HF token が job 全体の env にある | secret の過剰付与 |
| #445 | wan-lab の Docker image にローカルの秘密ファイルが入り得る | 情報漏えい |
| #451 | `EOF` という名前のファイルで GITHUB_OUTPUT を偽造できる可能性 | インジェクション |
| #269 | 無効化した plugin が外部 CLI コマンド経由で起動できる | 設定の迂回 |

### グレーの傾向

外部入力が届くかどうか issue からは判断できないもの（#317、#325、#387 など）、外部からは起動されないメモリ安全性違反（#154、#160、#388）、開発・評価スクリプトによる無関係プロセスの停止（#410、#416、#488 など）、製品は正しく動いているがセキュリティ検査のテストが弱いもの（#462、#484、#497 など）に大別される。

## 副次発見

### #441 は「ユーザー操作を条件とする任意コード実行」

`just/mesh-client.just` は次のように引数を bash スクリプトの文字列に埋め込む。

```just
mesh-client join="" port="9337" console="3131" config="":
    #!/usr/bin/env bash
    ...
        args+=(--join "{{ join }}")
```

just は `{{ join }}` をテキストとして置換してから bash に渡すため、値に含まれる `$(...)` は bash がコマンドとして実行する。ユーザーが自分のシェルで引用符を付けても防げない。issue の作成者は just 1.54.0 で、同じ書き方の fixture を使って展開されることを確認している。

- **成立条件**: 被害者が攻撃者の用意した文字列を自分で `just mesh-client` に渡して実行すること。ネットワーク越しの値が自動で recipe に流れる経路はない
- **悪用しやすさ**: 招待 token はもともと他人から受け取って貼り付けるもので、偽の token を渡すソーシャルエンジニアリングと相性がよい。docs や `.agents/skills/` にも `just mesh-client` の手順がある
- **影響範囲外**: CI と release workflow は `just release-bundle` を使わず、変数を引用符付きでスクリプトへ直接渡している。`mesh-llm --join <token>` を直接使う通常の利用者も影響を受けない
- **評価**: 当初の Medium/Low から、High 寄りの Medium へ引き上げた。修正は引数を `[positional-arguments]` か export した変数で渡すだけで済むため、優先して直す価値がある

### セキュリティの区分がない

474 件のどれにも `security` などのラベルがなく、脆弱性に該当する issue が P2・P3 の品質改善 issue と同じ扱いで並んでいる。公開 issue に再現手順付きで書かれているものもある（#365、#224 など）。

## 残課題

- 深刻度は本文からの推定で、実際に攻撃が成立するかは試していない。最優先の 3 件と #441 は、修正前に現行 main で再現を確かめるとよい
- 番号帯ごとの判定は分担して読んだ結果で、全件を通して再確認したわけではない。抜き取り確認は #170 と #378 のみ
- fork 元の `Mesh-LLM/mesh-llm` に同じ問題があるかは確認していない。ある場合は GitHub Security Advisory などの非公開経路での報告を検討する
- 該当 issue へのセキュリティ用ラベルの付与
