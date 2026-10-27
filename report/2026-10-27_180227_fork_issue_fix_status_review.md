# fork issue の脆弱性が手元コードで直っているかのレビュー — ほぼ未修正

- **実施日時**: 2026年10月27日 17:55 〜 18:04 JST (前回抽出した脆弱性 issue 42 件について、手元の main のコードを読んで修正状況を判定)
- **報告日時**: 2026年10月27日 18:04 JST
- **作成者**: Claude Opus 5.5

## 概要

前回のレポートでは、fork リポジトリの issue から脆弱性に当たるものを洗い出しました。今回はその続きとして、それらの問題が手元のコードで直っているかを一件ずつ確かめました。手元のコードは fork ではなく、fork 元である upstream の main です。

始める前に分かったことが一つあります。fork と upstream は途中で分かれ、それぞれ別々にコミットを積んでいます。issue が書かれた時点の fork のコードは手元にありません。逆に、fork には入っていない upstream 側の変更が手元にはあります。このため、fork で直したものが手元にはない場合も、upstream が別に直していた場合もあり得ます。その点に気をつけて判定しました。

作業は、対象を 3 つの領域に分けて並行してコードを読む形で進めました。領域は、本体の通信・課金・plugin 周り、モデル取得と cache 周り、CI と release scripts 周りの 3 つです。そのうえで、優先度の高い件と「直っている」と判定した件は、根拠の箇所を自分でも読んで確かめました。ビルドや攻撃の再現は行っていません。

結果として、ほとんどの問題は手元のコードにも残っていました。直っていたのは 3 件だけで、ほかに一部だけ直っているものが 2 件、対象のコードがもう無いものが 2 件ありました。前回最優先とした 3 件は、どれも直っていません。課金判定のずれ、Hugging Face の token が別のサーバに送られる問題、署名が無効でも release を通る問題の 3 つです。ディレクトリの外に書き込む問題も、4 件のうち直っていたのは Hugging Face の cache path の 1 件だけでした。開発者が招待 token を貼り付けたときにコマンドが動いてしまう `just` の問題も、そのまま残っています。

直っていた件も、issue を受けて直したものではありませんでした。upstream が fork と分かれた後に、別の目的で書き直したり削除したりした結果です。また、fork で修正済みとして閉じられている 6 件は、修正が手元には入っておらず、問題がそのまま残っています。そのうち 1 件は upstream が別のやり方で一部を直していました。

issue の記述より状況が悪いものも見つかりました。たとえば CI の自動修復の仕組みは、検証の処理が別の場所に移ったのに保護する範囲が変わっていないため、修復する側が検証器そのものを書き換えられる状態が続いています。ほかにも、runtime の取り込みは issue が想定したローカルの bundle だけでなく、ネットワークから取ってきた bundle でも同じ処理を通ります。

次は、最優先の 3 件と `just` の問題、ディレクトリ外への書き込みの 3 件から直すことを勧めます。fork で直したものは fork の修正をそのまま移植でき、`just` の問題も書き方を変えるだけで済むので、手間は大きくありません。また、手元のコードにも同じ問題があるので、upstream には非公開の経路で知らせることを検討してください。

## 添付ファイル

- [実装プラン](attachment/2026-10-27_180227_fork_issue_fix_status_review/plan.md)
- [42 件の修正状況と根拠の全件一覧](attachment/2026-10-27_180227_fork_issue_fix_status_review/fix_status_table.md)

## 核心発見サマリ

**結論**: 対象 42 件のうち、手元の main（`95f9b4810`）で修正済みは 3 件（#363、#408、#461）、一部修正 2 件（#68、#459）、該当コードなし 2 件（#445、#270）、**未修正 35 件**だった。High 5 件はすべて未修正（#170、#365、#474、#76、#1）。修正済みの 3 件はどれも、upstream が 2026-10-12〜13 に入れた変更（ef694d419、967316821 + f5273fd1e、be4005c55）による。fork が分岐した後の upstream の変更で、issue を受けた修正ではない。fork で CLOSED の 6 件（#1、#2、#65、#68、#76、#371）は、fork の修正 PR（#21、#78、#82、#94、#547）がどれも手元にない。

| 深刻度 | 件数 | 修正済み | 一部修正 | 該当コードなし | 未修正 |
|---|---|---|---|---|---|
| High（High〜Medium を含む） | 5 | 0 | 0 | 0 | 5 |
| Medium（#441 を含む） | 18 | 1 | 2 | 1 | 14 |
| Low〜Medium | 5 | 0 | 0 | 0 | 5 |
| Low | 14 | 2 | 0 | 1 | 11 |
| **計** | **42** | **3** | **2** | **2** | **35** |

## 前提・目的

- **背景**: [前回レポート](./2026-10-27_130344_fork_issue_security_triage.md)で、fork `jimoto-no-llm/mesh-llm` の issue から脆弱性 42 件を抽出した
- **目的**: その 42 件が、手元のコード（upstream `Mesh-LLM/mesh-llm` の main）で直っているかを判定する
- **方法**: コードを読むだけの静的レビュー。ビルド・テスト・攻撃の再現はしていない
- **注意点**: fork と upstream は分岐している。issue が参照している fork のコミット `18142efb0` は手元にない。判定は「手元のコードで、issue が指摘した処理が今どうなっているか」を基準にした

## 環境情報

- 手元のブランチ: `docs/fork-issue-security-triage`。`main` の `95f9b4810`（2026-10-22）にレポートを足しただけで、コードの差分はない
- issue が参照している fork のコミット: `18142efb0`（2026-10-16）
- fork と upstream の分岐: merge-base は `43ddd24e6`（2026-10-10）。fork だけにあるコミットが 70、upstream だけにあるコミットが 154（2026-10-27 時点の GitHub compare API による）
- issue 本文: 前回の添付 `issues.json`

## 参照レポート

- [fork リポジトリ issue の脆弱性洗い出し](./2026-10-27_130344_fork_issue_security_triage.md)

## 再現方法

1. 前回の添付 `issues.json` から、対象 issue の本文を読む
2. 本文が指摘している関数や処理を、手元のコードで探して読む。ファイル名や関数名ではなく、実際の呼び出し経路で判断する
3. 直っている場合は、`git log -S` や `git log -L` で、直したコミットを探す
4. fork で CLOSED の issue は、`gh pr view -R jimoto-no-llm/mesh-llm` で fork の修正 PR を調べ、その merge commit とファイルが手元にあるかを `git cat-file` で確かめる
5. 3 つの領域を並行して読み、結果を統合する。そのあと、High の件、#441、path traversal の件、修正済みと判定した件は、根拠の箇所を自分で読んで裏付けた（#170、#365、#474、#441、#363、#359、#384、#224、#1、#456、#408）

## 結果詳細

全 42 件の根拠は [fix_status_table.md](attachment/2026-10-27_180227_fork_issue_fix_status_review/fix_status_table.md) にある。ここでは High、Medium、修正済みの件を挙げる。

### High（すべて未修正）

| # | 判定 | 今のコード |
|---|---|---|
| #170 | 未修正 | `ingress/execution.rs:16` で公開 ID を内部名に書き換えてから、`response/routing.rs:92-97` が書き換え後の名前を価格表と完全一致で照合する。paid server 側の `payments/server.rs:125-133` は、要求名と backend 名の両方で価格を引く。照合キーが食い違うため、価格を公開 ID で設定している構成では、remote 経路が支払いなしで通る可能性がある |
| #365 | 未修正 | `skippy-hf-hub/src/pagination.rs:68-71` は毎ページ同じ認証ヘッダを付ける。`parse_link_header_next`（`:115-125`）は Link ヘッダの URL を origin 照合なしで返す |
| #474 | 未修正 | `tools/xtask/src/attestation.rs:430-439` の `inspect` は、status が missing/invalid でも成功で終わる。composer（`scripts/ci-compose-product-input.sh:105-108`）と host producer 2 本は、出力の status を見ない。status を検査する `product attestation-status` は既にあるが、使っているのは `package-release` の通常経路だけ |
| #76 | 未修正 | `StageStopRequest`（`stage/types.rs:156-162`）に送信元 coordinator の情報がない。fork の修正 PR #94 は手元にない |
| #1 | 未修正 | `request_parse.rs:799-811` と `:837-839` は、重複する Content-Length を後勝ちで受理し、TE と CL が両方ある要求も受理する。fork の修正 PR #21 は手元にない |

### Medium

| # | 判定 | 今のコード |
|---|---|---|
| #441 | 未修正 | `just/mesh-client.just:4-14` は `{{ join }}` などを bash recipe の本文に文字列として埋め込む。`release-bundle.just` も同じ。`[positional-arguments]` も export も使っていない |
| #363 | **修正済み** | `repository/download.rs:537-541` で、ETag を `admit_etag`、commit を `admit_commit`（40 桁 hex）で検証してから path を作る（ef694d419） |
| #359 | 未修正 | `buckets/sync.rs:462` と `:689` は、remote path を検証せずに local root に join して書き込む |
| #384 | 未修正 | `native-runtime/src/cache.rs:111-112` は、id と release_version を検証せずに join する。`skippy-runtime-install/src/install.rs:415-438` のネットワークからの取り込みも、同じ経路を通る |
| #224 | 未修正 | `mesh-llm-skills/src/lib.rs:116-133` と `:200` は、skill 名を検証せずに join する。`is_valid_skill_name` は nested skill の経路でしか呼ばれない |
| #68 | 一部修正 | upstream が別に再実装し、reasoning の保留と ID の不一致は解消した（8b848ead5）。バッファに上限がない問題は残る |
| #459 | 一部修正 | smoke の Python 導入では HF token を空にしている（be4005c55）。`claude-live-model-gate.yml` の API key と、GPU job の `HF_TOKEN` は、今も job env にある |
| #445 | 該当コードなし | wan-lab は削除された（496eec6a5）。ルートの `.dockerignore` は今も `.env` を除外しない |
| #455 | 未修正 | 検証器は `tools/xtask`（`cargo xtool`）に移ったが、保護する pathspec は移っていない（`agentic-replay-repair.sh:96`） |
| #192、#153、#175、#361、#319、#358、#475、#451、#269 | 未修正 | 指摘された処理が今もそのまま残っている（根拠は全件一覧） |

### 修正済み（3 件）と該当コードなし（2 件）

| # | 判定 | 理由 | コミット |
|---|---|---|---|
| #363 | 修正済み | ETag と commit を検証するようになった | ef694d419（#2323） |
| #408 | 修正済み | Python 版が Rust に移り、`safe()` と `ensure_directory` でパス成分を検査するようになった（`competitive_matrix.rs:246-277`、`:357-374`） | 967316821（#2360）、f5273fd1e（#2377） |
| #461 | 修正済み | ハッシュを固定した lock ファイルから `--require-hashes` で導入するようになった | be4005c55（#2374） |
| #445 | 該当コードなし | wan-lab ごと削除された | 496eec6a5（#2461） |
| #270 | 該当コードなし | Python SDK が外部リポジトリに移った | f5273fd1e（#2377） |

## 副次発見

### issue の記述より状況が悪いもの

- **#455**: 検証が Python スクリプトから `cargo xtool`（`.cargo/config.toml` で `run --locked -p xtask`）に移った。そのため、修復対象のツリーにある `tools/xtask` が、そのままビルドされて検証器として動く。保護している pathspec は `.github .agents scripts evals ci` のままで、`tools/`、`.cargo/`、`Cargo.toml` / `Cargo.lock` は保護されていない
- **#384**: issue はローカルの bundle だけを想定しているが、リモート catalog から取ってきた bundle の取り込み（`skippy-runtime-install/src/install.rs:415-438`）も同じ `install_from_dir` を通る。CLI の `remove`（`skippy-commands/src/runtime.rs:172`）も、ユーザーが入力した id をそのまま `remove_dir_all` に渡す
- **#76**: `mesh/stage_artifacts.rs:318-321` は、Stop を受け付けるか判定する前に、transport bridge を無条件で止める。そのため、拒否されるはずの Stop でも bridge が消える。fork PR #73 の修正も手元にはない

### 同じ種類の別の問題

- **#441 の仲間**: `just/release-bundle.just` の `release-attestation *ARGS` recipe も `{{ ARGS }}` を埋め込んでいて、`[positional-arguments]` が付いていない
- **#445 の仲間**: `tools/oss-scanner/Dockerfile:29` は `COPY . /src` を使っており、ルートの `.dockerignore` は `.env`、`.envrc`、鍵ファイルなどを除外していない。build context が repo root かどうかは確かめていない
- **#224 の仲間**: plugin 名の検査 `plugin-manager/src/source_ref.rs:269-274`（`is_valid_name`）は、`.` や `..` のような、記号だけでできた名前を拒否しない可能性がある。root skill の経路は plugin 名をそのまま skill 名として使うので、確かめる必要がある
- **#175 の仲間**: `openai/transport.rs:1779-1784` は endpoint の URL をそのまま info ログに出す

### fork の修正は upstream に戻っていない

fork で CLOSED の issue の修正 PR（#21、#73、#78、#82、#94、#547）は、どれも手元の履歴にない。fork だけに追加されたファイル（`request_parse/framing.rs`、`responses_stream.rs` など）も手元にはない。逆に、#363・#408・#461 を直した upstream のコミットは fork が分岐した後のもので、fork には入っていない。issue はどちらか一方だけで直っていて、両方で直っているものはない。

### 件数の訂正

前回のレポートでは「該当 43 件」としたが、一覧表（`triage_table.md`）にある行は 42 件だった（High 5、Medium 18、Low〜Medium 5、Low 14）。前回の本文にある「Low〜Medium / Low 20 件」は、表では 19 件しかない。今回は表にある 42 件を対象にした。数え間違いか表の書き漏れかは、まだ確かめていない。

## 残課題

- 判定はコードを読んだだけ。最優先の件（#170、#365、#474、#441）は、修正する前にテストで再現を確かめるとよい
- 直す順番の案:
  - #474: composer と host producer で `product attestation-status` を使う
  - #365: origin の照合を入れる。fork にある `location_stays_on_hub` の判定が流用できる
  - #441: `[positional-arguments]` に切り替える
  - #359、#384、#224: パス成分を検証する
  - #170: 価格の照合キーを揃える
  - #455: 保護する pathspec を広げる
- fork で直した #1、#2、#65、#76、#371 は、fork の PR を upstream に移植すれば直る。#371 を移植しても、同じ関数にある #372 は残る
- 前回の件数のずれ（43 と 42）の原因を確かめる
- #270 は外部リポジトリ `Mesh-LLM/mesh-llm-python-sdk` で、checksum の検査が有効になっているかを確かめる
- 手元の upstream にも同じ問題があるので、GitHub Security Advisory などの非公開の経路で upstream に知らせることを検討する
