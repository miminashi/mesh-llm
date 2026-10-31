# 直接 packaging の checksum 照合漏れ（issue #20）— fork では使わないので close

- **実施日時**: 2026年10月31日 17:30 〜 17:47 JST (解説コメントの投稿、package-release の呼び出し元の調査、CLAUDE.md への追記、issue の close)
- **報告日時**: 2026年10月31日 17:48 JST
- **作成者**: Claude Opus 5.5

## 概要

issue #20 は、配布用のアーカイブ（bundle）を組み立てるスクリプトの問題を扱っている。推論用のライブラリ群（native runtime）には、中のファイルの checksum を書いた manifest が付いている。ところが、bundle をその場で組み立てる経路では、この manifest と実際のファイルを照合しない。そのまま今のファイルから新しいハッシュを作り、正式な digest として記録してしまう。このため、壊れたり書き換えられたりしたライブラリが入っていても、正規品として扱われる bundle ができてしまう。組み立て済みの bundle を受け取るもう一方の経路では、照合をしている。

まず、ユーザーの依頼で、この問題を噛み砕いて説明するコメントを issue に書いた。manifest を「封印付きの納品書」、bundle の digest を「出荷ラベル」にたとえ、照合せずに今の中身からラベルを作るのがなぜまずいかを説明した。あわせて、既存の検査スクリプトを 1 行呼ぶだけで直せることも書いた。

次に、ユーザーから「この fork ではそもそも配布用アーカイブを組み立てないので、解決済みの issue #3 と同じように回避できないか」という提案があった。そこで、このスクリプトがどこから呼ばれているかを調べた。呼び出し元は、#3 で無効にした release の workflow と、手で実行しない限り動かない 2 つの just のコマンドだけだった。ほかの CI の workflow からは呼ばれていない。aws-v100 での普段の運用も、ビルドしたバイナリをそのまま起動していて、bundle は作っていない。

この問題が実害になるのは、作った bundle を他のノードに配り、digest で正規品かどうかを判断させる場合だけだ。この fork ではそういう配布をしないので、コードは直さずに閉じられると判断した。

対応として、CLAUDE.md の注意事項に、bundle を作る just のコマンドを使わないことと、使う必要が出たらこの issue を開き直すことを書いた。issue には閉じる理由と将来の直し方をコメントし、not planned で閉じた。

どの判断もコードを読んだうえでのもので、壊れたライブラリで実際に bundle を作って再現したわけではない。将来この fork で bundle を作ることになったら、その前に issue を開き直して修正する必要がある。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_174839_issue20_direct_packaging_checksum_not_planned/plan.md)
- [投稿した解説コメント](attachment/2026-10-31_174839_issue20_direct_packaging_checksum_not_planned/explanation-comment.md)
- [投稿した close 理由のコメント](attachment/2026-10-31_174839_issue20_direct_packaging_checksum_not_planned/close-comment.md)

## 核心発見サマリ

**結論**: `package-release.sh` の呼び出し元は、`release.yml`（8 か所、#3 で `disabled_manually`）、`just bundle`（`just/mesh.just`）、`just release-bundle`（`just/release-bundle.just`）の 3 つだけで、release 以外の workflow からは呼ばれていない（`compute-changes` が変更検知のパターンにファイル名を入れているだけ）。aws-v100 の運用（`just build cuda 70` → `target/debug/mesh-llm serve`）はこの経路を通らない。コードは変更せず、CLAUDE.md に注意書きを足して、issue #20 を `NOT_PLANNED` で閉じた。

## 前提・目的

- **背景**: issue #20 は jimoto-no-llm/mesh-llm#475 を移したもの。[Medium の脆弱性課題の残存確認](./2026-10-31_172648_medium_security_issues_recheck.md)で「残存」と判定して起票した
- **目的**: 課題を分かりやすく説明し、fork での対応を決める
- **前提条件**: この fork は限られたメンバーで使い、release も bundle の配布も行わない（#3 で release の workflow を無効にした）

## 環境情報

- fork: `miminashi/mesh-llm` main `007d052d8`

## 結果詳細

### 問題の仕組み

| 経路 | 流れ | runtime の checksum 照合 |
|---|---|---|
| precomposed（`package-release.sh:495-502`） | `verify-native-runtime-package.sh` → `product compose --check` | あり |
| 直接（`package-release.sh:620-629`） | `native select-runtime` → `cp -R` → `product compose` | なし |

- `select_native_runtime_dir`（`:130-146`）は OS・arch・backend・CUDA のメジャーバージョンで runtime を選ぶだけ
- `tools/xtask/src/product/compose.rs:98-99` は `tree_sha256` で今の tree から digest を作るだけで、runtime manifest の checksum とは比べない
- 検査スクリプトの実体は `scripts/verify-native-runtime-package.sh` → `skippy/scripts/verify-native-runtime-package.sh` → `mesh_automation native verify-runtime-package`

### package-release の呼び出し元

| 呼び出し元 | 状態 |
|---|---|
| `.github/workflows/release.yml`（504、1123、1244、1324、1396、1466 行の `.sh`、1631、1685 行の `.ps1`） | #3 で `disabled_manually` |
| `just bundle`（`just/mesh.just:46-52`） | 手動のみ |
| `just release-bundle`（`just/release-bundle.just`） | 手動のみ |
| `.github/actions/compute-changes/derive-outputs.sh` | 変更検知の正規表現にファイル名があるだけで、実行はしない |

`tools/xtask` のテストや `release_targets.rs` にも名前は出てくるが、スクリプトの内容や整合性を確かめるためのもので、bundle を配布する経路ではない。

### 行った対応

- issue に解説コメントを投稿した（[comment](https://github.com/miminashi/mesh-llm/issues/20#issuecomment-6541201480)）
- CLAUDE.md の「注意事項」に次の 1 行を足した:
  > 配布用アーカイブを作る `just bundle` / `just release-bundle`（中身は `scripts/package-release.sh`）は使わない。直接の経路では native runtime の checksum を照合せずに digest を付けてしまう（issue #20、未修正で close）。使う必要が出たら、先に #20 を開き直して直す
- close の理由をコメントし（[comment](https://github.com/miminashi/mesh-llm/issues/20#issuecomment-6541315576)）、`not planned` で閉じた（`stateReason: NOT_PLANNED`）

## 参照レポート

- [attestation の検査漏れ（issue #3）— fork では release を止めて close](./2026-10-31_023229_issue3_disable_release_workflows.md)
- [Medium の脆弱性課題の残存確認と起票](./2026-10-31_172648_medium_security_issues_recheck.md)

## 残課題

- 将来 release の workflow を戻すか、`just bundle` / `just release-bundle` を使うときは、先に #20 を開き直す。直し方は、直接の経路で `cp -R` の前に `"$SCRIPT_DIR/verify-native-runtime-package.sh" "$runtime_dir"` を呼ぶこと
- CLAUDE.md は git で管理されていない（未追跡）ため、追記はコミットしていない
