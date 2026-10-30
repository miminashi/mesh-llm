# attestation の検査漏れ（issue #3）— fork では release を止めて close

- **実施日時**: 2026年10月31日 02:00 〜 02:35 JST (上流での対応状況の再調査、issue への追記と解説、release workflow の無効化、issue の close)
- **報告日時**: 2026年10月31日 02:35 JST
- **作成者**: Claude Opus 5.5

## 概要

issue #3 は、release 用のバイナリに埋め込まれる署名付きの証明（release attestation）が無かったり壊れていたりしても、release の手順が止まらない、という問題だった。確認する仕組みはあるのに、結果を受け取る側が「確認が正常に終わったか」しか見ておらず、「確認の結果が有効か」を読んでいなかった。

まず、上流でこの問題が直っているかを調べ直した。該当するファイルは issue を書いた時点から一度も変わっておらず、上流の issue と PR を検索しても、この問題を扱うものは無かった。名前が似ている open の PR も中身まで確かめたが、別の問題を扱っていた。

調べる途中で、issue に書いた内容よりも影響が大きいことが分かった。issue には「最後の packaging の段階では結果を確かめている」と書いていた。しかし実際の release の workflow では、「前の段階で確認済み」という設定が入っていて、この確認も飛ばされていた。つまり release のどの段階でも、証明が有効かを実際には確かめていなかった。これを issue に追記した。あわせて、ユーザーの依頼で、課題を噛み砕いて説明するコメントも書いた。

次に、どう対応するかを決めた。この fork は限られたメンバーだけで使い、広く release する予定はない。上流に修正を送る予定もない。ユーザーからは「attestation 自体を無効にしてはどうか」という案が出た。検討の結果、コードから attestation を消すのではなく、release の workflow を止めることにした。この問題は release の workflow の中でしか起きないうえ、コードを消すと上流を取り込むたびに衝突が起き、ピアを mesh に参加させる判定にも影響が広がるからだ。

対応として、release 系の 2 つの workflow を無効にした。issue には対応の内容と、将来 release するときに開き直すべきことをコメントし、not planned で閉じた。手元ビルドには release の署名が無いので、mesh を作るときに証明を必須にする設定を使うと、メンバーのノードどうしが参加できなくなる。この注意点を CLAUDE.md に書いた。

どの判断もコードを読んだうえでのもので、無効なバイナリで release を流して再現したわけではない。将来この fork から release を行う場合は、workflow を戻す前に issue を開き直して修正する必要がある。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_023229_issue3_disable_release_workflows/plan.md)

## 核心発見サマリ

**結論**: 上流（`Mesh-LLM/mesh-llm` main `61e2acfb9`、2026-10-31 時点）は未対応で、関係する PR も無い。さらに、`release.yml` の 8 か所で `MESH_RELEASE_ATTESTATION_PREVERIFIED: "1"` が設定されている。このため `package-release.sh` / `.ps1` も `product attestation-status` を呼ばず、release のどの段階でも status が valid かを確かめていない。fork では `release.yml` と `resume-crates-release.yml` を `disabled_manually` にし、issue #3 を `NOT_PLANNED` で閉じた。コードは変えていない。

## 前提・目的

- **背景**: issue #3 は jimoto-no-llm/mesh-llm#474 を移したもの。前回の調査（[fork issue の修正状況レビュー](./2026-10-27_180227_fork_issue_fix_status_review.md)）でも未修正と判定していた
- **目的**: 上流での対応状況を調べ直し、fork での対応を決める
- **前提条件**: この fork は限られたメンバーで使う。広く release せず、上流に修正も送らない

## 環境情報

- fork: `miminashi/mesh-llm` main `7164e14b0`（関係するファイルの中身は upstream/main と同じ）
- upstream: `Mesh-LLM/mesh-llm` main `61e2acfb9`

## 結果詳細

### 上流のコード

| 箇所 | 状態 |
|---|---|
| `tools/xtask/src/attestation.rs:430-439`、`:457-482` | status が missing / invalid でも `Ok` を返す。`286bd1aea` から変わっていない |
| `scripts/ci-compose-product-input.sh:105-108` | `inspect --json` を呼ぶだけ |
| `.github/actions/prepare-host-input/action.yml:112` | 同じ |
| `.github/actions/prepare-windows-host-input/action.yml:145-150` | `$LASTEXITCODE` しか見ない |
| `mesh/scripts/package-release.sh:201-203`、`package-release.ps1:318-320` | PREVERIFIED=1 だと "verified by immutable product composer" と出力して検査を飛ばす |
| `.github/workflows/release.yml` 502、1121、1242、1322、1394、1464、1627、1683 行 | `MESH_RELEASE_ATTESTATION_PREVERIFIED: "1"`。#1113 で入り、#2079 で今の形になった |

### 上流の issue と PR

`attestation inspect`、`attestation-status`、`release-attestation`、`attestation exit code`、`attestation composer`、`prepare-host-input attestation` で、issue と PR を状態を問わず検索した。この問題を扱うものは無かった。

- #2641（open）`ci(windows): catch release packaging failures in PR and main CI`: PREVERIFIED のモードを PR と main の CI にも使う。composer の検査は変えていない
- #2566（open）`feat(mesh): share one loaded model across independent meshes`: ランタイムの `release_attestation.rs` で、自分の attestation を読み込んでログに出す処理だけを変えている
- jimoto-no-llm/mesh-llm#474: open のままで、コメントも修正 PR も無い

### fork で行ったこと

- issue #3 へのコメント
  - [上流の状況と PREVERIFIED の追記](https://github.com/miminashi/mesh-llm/issues/3#issuecomment-6528990431)
  - [課題の噛み砕いた解説](https://github.com/miminashi/mesh-llm/issues/3#issuecomment-6529065160)
  - [対応と close の理由](https://github.com/miminashi/mesh-llm/issues/3#issuecomment-6529158583)
- `gh workflow disable release.yml` / `resume-crates-release.yml`。どちらも `disabled_manually` になったことを確認した
- `gh issue close 3 --reason "not planned"`。状態は `CLOSED` / `NOT_PLANNED`
- `CLAUDE.md` の「注意事項」に、`--require-release-attestation`（`mesh-llm-cli/src/parser/top_level_options.rs:201-203`）と `mesh_requirements.require_release_attestation`（`mesh-llm-config/src/model.rs:314`、既定値は false）を使わないこと、release の workflow を無効にしたことを書いた。CLAUDE.md は未追跡のファイルなので、コミットしていない

## 副次発見

- v100 の `~/.mesh-llm` には `require_release_attestation` の設定が無い。起動コマンドにも `--require-release-attestation` は無い。今の mesh は、attestation を必須にしていない
- `gh workflow list --all` の出力には、release 系の workflow が出てこなかった。状態は `gh api repos/miminashi/mesh-llm/actions/workflows` で確かめた

## 残課題

- この fork から release を行うことになったら、workflow を `gh workflow enable` で戻す前に、issue #3 を開き直して composer と host producer を直す
- `release.yml` は upstream の取り込みで中身が変わっても、無効のまま。新しく増えた release 系の workflow があれば、その都度止める必要がある
