# 脆弱性トリアージ全件一覧

対象: `jimoto-no-llm/mesh-llm` の全 issue 474 件（2026-10-23 取得）。深刻度は issue 本文に書かれた攻撃者・前提条件からの推定で、実際の攻撃は試していない。

## 該当（42 件）

| # | 状態 | 深刻度 | 分類 | 要点 |
|---|---|---|---|---|
| #170 | OPEN | High | 課金ゲートの迂回 | 公開 model ID を内部名に書き換えた後で価格を照合するため、remote peer が有料 model を支払いなしで推論させられる可能性（P1） |
| #365 | OPEN | High | token 漏えい | HF 一覧の pagination で、別 origin の `next` Link にも Bearer token を送る。loopback の別 port でダミー token の受信を確認済み |
| #474 | OPEN | High | 署名検証の迂回 | `release-attestation inspect` が missing/invalid でも終了コード 0。composer と host producer は終了コードしか見ないため、改ざんされた host バイナリが release に入り得る |
| #76 | CLOSED | High | 認可の欠如 | Stage Stop 要求に coordinator の本人性がなく、任意の mesh peer が他ノードのステージを停止できた |
| #1 | CLOSED | High〜Medium | HTTP request smuggling | Content-Length の重複や TE と CL の併存を受理し、元のフレーミングヘッダのまま上流へ転送していた |
| #441 | OPEN | Medium（High 寄り） | コマンドインジェクション | `mesh-client` と `release-bundle` の recipe が引数を bash ソースへ文字列補間する。細工した join token で、実行したユーザーの権限でコマンドが動く（本文「副次発見」参照） |
| #363 | OPEN | Medium | path traversal | HF の ETag・X-Repo-Commit を検証せず blob/lock/snapshot path に join する |
| #359 | OPEN | Medium | path traversal / symlink | bucket sync が remote listing の path を検証せず local root へ書き込む |
| #384 | OPEN | Medium | path traversal | runtime manifest の id・release_version を検証せず、細工 bundle の import で cache 外へコピーされ得る |
| #224 | OPEN | Medium | path traversal | 公開関数 `install_skills` が skill 名を検証せず、Skill root 外へ書き込める（再現確認済み） |
| #192 | OPEN | Medium | symlink 追従 | ログ初期化の固定名 `.write_test` が symlink を辿り、参照先ファイルを空にする |
| #153 | OPEN | Medium | TOCTOU / 鍵管理 | node 鍵を検査した後に path を開き直すため、親ディレクトリを変更できる actor が鍵をすり替えられる |
| #175 | OPEN | Medium | 情報漏えい | 先頭に空白のある endpoint URL の credential が metrics label に残り、`GET /api/status` から読める |
| #361 | OPEN | Medium | DoS | server が申告したサイズのまま body を読む前に `BytesMut::with_capacity` する |
| #319 | OPEN | Medium | DoS | binary stage の受信が接続ごと 32MiB の上限を超え、1 frame 512MiB まで確保する |
| #358 | OPEN | Medium | 整合性 | resume で HEAD と GET の identity を照合せず、新旧の内容を継いだファイルが旧 commit として確定する |
| #475 | OPEN | Medium | 整合性 | 直接 packaging が既存 checksum と照合せずに新しい digest を付け直す |
| #455 | OPEN | Medium | CI の整合性 | Replay repair の保護 pathspec が移動後の検証プログラムを含まず、修復 agent が検証器を書き換えられる |
| #459 | OPEN | Medium | secret の過剰付与 | Claude API key と HF token が job 全体の env にあり、未固定の依存 install step にも渡る |
| #445 | OPEN | Medium | 情報漏えい | wan-lab の Docker build が repo root を `COPY . .` し、`.env` などのローカル秘密ファイルを除外していない |
| #451 | OPEN | Medium | インジェクション | GITHUB_OUTPUT の固定区切り `EOF` が、同名のファイルを含む PR で破られ、output を偽造できる可能性 |
| #269 | OPEN | Medium | 設定の迂回 | config で無効化した plugin が、外部 CLI コマンド経由で起動できる |
| #68 | CLOSED | Medium | DoS | `/v1/responses` メッシュ中継の状態バッファに上限がなかった |
| #65 | CLOSED | Low〜Medium | 情報漏えい | Proxy-Authorization などの hop-by-hop ヘッダが上流へ素通しされていた |
| #266 | OPEN | Low〜Medium | 情報漏えい | `--relay-auth` の値が Clap の解析エラーにそのまま出る |
| #145 | OPEN | Low〜Medium | DoS / 鍵管理 | keystore の Argon2 パラメータに上限がなく、ファイルの書換えで巨大なメモリ確保を強制できる |
| #176 | OPEN | Low〜Medium | 整合性 | u64 を超える整数 literal が f64 に丸められたまま paid request の digest が付く |
| #370 | OPEN | Low〜Medium | symlink 追従 | catalog cache の prune が directory symlink を辿り、外部の JSON を削除する |
| #264 | OPEN | Low | 情報漏えい | download analytics に catalog 外の path 風入力がそのまま送られる |
| #339 | OPEN | Low | DoS | compose-mtp の GGUF reader が count をそのまま `Vec::with_capacity` に渡す（ローカル CLI） |
| #373 | OPEN | Low | DoS | cache 一覧の走査で directory symlink の循環を打ち切らない |
| #372 | OPEN | Low | DoS | bucket 日時の無効な month/day で符号なし underflow による panic |
| #371 | CLOSED | Low | DoS | bucket 日時の非 ASCII 入力で固定 slice が panic |
| #2 | CLOSED | Low | DoS | ヘッダと chunked の上限を Complete 経路で検査していなかった（超過は約 1 read 分） |
| #408 | OPEN | Low | path traversal | competitive benchmark の config で出力ルート外のディレクトリを移動できる（評価ツール） |
| #270 | OPEN | Low | 整合性 | Python UniFFI binding の API checksum 検査がコメントアウトされている |
| #315 | OPEN | Low | TOCTOU | 複数の取得が同じ partial ファイルへ同時に追記できる（完成時の SHA 検証で拒否されるため DoS 止まり） |
| #204 | OPEN | Low | なりすまし | MCP の tool・prompt 名を escape せずに連結し、名前を工夫した plugin が別 plugin の tool を上書きできる |
| #456 | OPEN | Low | 認可境界 | 手動全体 CI の guard が短い ref 名だけを比べ、同名の tag も通る |
| #461 | OPEN | Low | サプライチェーン | CI smoke の pip 依存がバージョン未固定（#459 により HF token を継承） |
| #498 | OPEN | Low | サプライチェーン | nightly の agent client を `@latest` や移動する installer URL から導入 |
| #541 | OPEN | Low | サプライチェーン | docs が `mermaid@11` を CDN から exact version・integrity なしで import |

## グレー（脆弱性とは言い切れないもの）

| 区分 | issue |
|---|---|
| 外部入力が届くかどうか不明 | #317（stage identity の区切り衝突）、#325（topology 探索に上限がない）、#320（orphan session の回収漏れ）、#380（K のみの MLA で null 参照）、#387（入れ子の空 grammar で制約が外れる）、#366（Xet URL の revision 未 encode）、#382、#381、#202、#178、#172、#177、#207、#146、#138 |
| メモリ安全性（外部からは起動されない） | #154、#160、#388（multi-thread 起動後の `set_var`） |
| 鍵・ファイル置換の耐障害性 | #143、#144、#140、#141 |
| 整合性検証の不足（攻撃者モデルが弱い） | #349、#368、#439、#369、#342、#343、#409、#406、#16、#66、#480、#537、#189 |
| 棚卸しタスク | #64（外部入力経路に残る panic） |
| 開発・評価スクリプトの問題 | #410、#416、#488、#506、#481、#403、#401、#435、#425、#292、#296、#337、#340、#346 |
| セキュリティ検査のテスト不備（製品は正しく動いている） | #462、#484、#497、#471、#527、#485、#453、#195 |
| その他 | #184/#185（Nostr score の符号変換）、#241（ログ削除範囲）、#268（wallet の ledger 選択）、#272（不正な join token が未設定扱い）、#313（tool_choice の検証） |
