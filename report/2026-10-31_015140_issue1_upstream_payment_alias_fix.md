# 課金判定の照合キーずれ（issue #1）— 上流で修正済みのため close

- **実施日時**: 2026年10月31日 01:30 〜 01:52 JST (上流での対応状況の再調査、取り込み済み修正のテスト実行)
- **報告日時**: 2026年10月31日 01:52 JST
- **作成者**: Claude Opus 5.5

## 概要

issue #1 は、remote のピアから届いた推論要求について、価格を公開 model ID で設定している seller が支払いなしで推論してしまうおそれがある、という問題だった。当初の方針は、上流で直るまで課金機能（`payments` feature）を既定のビルドから外しておき、上流で直ったら元に戻して issue を閉じる、というものだった。

実装に取りかかる前に上流の状況を改めて調べたところ、上流にもまったく同じ問題が issue として立っており、それを直す PR がすでにマージされていた。その修正は、上流の main へまとめて入った大きなバグ修正 PR に含まれており、fork の main にも先日の upstream マージで取り込まれていた。

修正後のコードは、書き換え後の内部の実行名と、公開 model ID（profile 付き・無しの両方）のどちらかに価格があれば支払いを求める。issue #1 に書いた「期待する動作」と一致しており、古い完全一致だけの判定はコードから消えている。

念のため、修正と一緒に入った単体テストをワークステーションで実行し、すべて通ることを確かめた。aws-v100 で動いている mesh-llm も、この修正を含むコミットからビルドされたものだった。

以上から、課金機能を既定から外す変更は不要になったと判断し、ユーザーの了承を得て、コードは変更せずに issue #1 を閉じる。課金機能は既定のまま有効だが、この fork では価格を設定していないので、もともと影響はない。

## 添付ファイル

- [実装プラン](attachment/2026-10-31_015140_issue1_upstream_payment_alias_fix/plan.md)
- [payment_admission テストログ](attachment/2026-10-31_015140_issue1_upstream_payment_alias_fix/payment_admission_test.log)

## 核心発見サマリ

**結論**: 上流 issue Mesh-LLM/mesh-llm#2246（同一問題）は PR #2598 で修正され、PR #2614（`b8b0da343`, 2026-10-29 22:06 UTC）経由で upstream main に入っている。fork main（`fa2d45afd`）は `b8b0da343` を含み、seller 判定 `response/routing.rs:50` は新しい `payment_admission::requires_payment` を呼ぶ。関連テスト 4 件が pass（0 failed）。v100 の稼働バイナリ（`0.78.0+g46F814`）も修正を含む。`payments` を既定 feature から外す変更は行わない。

## 前提・目的

- **背景**: issue #1（`286bd1aea` 時点のコード読みで発見）。方針は「上流修正まで `payments` を既定から外す」
- **目的**: 上流の対応状況（PR 含む）を再調査し、方針を決め直す

## 環境情報

- ワークステーション、fork main `fa2d45afd`（upstream/main との差分 1 コミット）
- テスト: `cargo test --locked -p mesh-llm-host-runtime --lib payment_admission`（`just` 未導入のため素の cargo。sccache 無効、`MESH_LLM_SKIP_UI=1`）

## 結果詳細

### 上流の対応状況

| 項目 | 状態 |
|---|---|
| [Mesh-LLM/mesh-llm#2246](https://github.com/Mesh-LLM/mesh-llm/issues/2246) Payment admission compares the rewritten internal model name against operator pricing keys | CLOSED (completed), 2026-10-29 |
| [PR #2598](https://github.com/Mesh-LLM/mesh-llm/pull/2598) fix(runtime): enforce seller pricing under a model's public alias | MERGED（base: `fix/megabugs`）, 2026-10-26 |
| [PR #2614](https://github.com/Mesh-LLM/mesh-llm/pull/2614) fix: big aggregating branch for capturing bug fix work | MERGED → upstream main `b8b0da343`, 2026-10-29 |

PR #2598 のマージコミット `76939e6b` 自体は fork main の祖先ではない（`fix/megabugs` を経由したため）。修正の取り込みは `payment_admission.rs` の git log が `b8b0da343` を指すことで確認した。

### 修正内容（`mesh/crates/mesh-llm-host-runtime/src/network/openai/response/payment_admission.rs`）

- `requires_payment`: 価格表が空なら false。それ以外は served descriptor から `public_identities_for_model` で公開 ID（profile 付き／profile 無し）を求め、要求の model 名かいずれかの公開 ID が価格表にあれば支払いを求める
- `response/routing.rs:50` の seller 判定（ローカル起点以外）がこれを呼ぶ。旧来の完全一致のみの `priced_model_requires_payment` 呼び出しは他に残っていない

### テスト結果

```
payment_admission::tests::decoded_chunked_model_cannot_bypass_seller_payment_admission ... ok
payment_admission::tests::unprofiled_public_alias_requires_payment_after_profile_fallback ... ok
payment_admission::tests::priced_public_alias_still_requires_payment_after_ingress_alias_rewrite ... ok
auto_route::payment_admission::tests::mixed_replicas_measure_readiness_only_on_admitted_targets ... ok
test result: ok. 4 passed; 0 failed
```

### aws-v100

`/api/status` の version は `0.78.0+g46F814`。`46f814719` は `b8b0da343` を含む。

## 結論・対応

- `payments` を既定 feature から外す変更（`mesh-llm` / `mesh-llm-host-runtime` の `Cargo.toml`、`build-host.sh`）は行わない
- issue #1 に調査結果をコメントして close する

## 副次発見

- ワークステーションには `just` が入っていない（v100 にはある）。今回は fork CI と同じ素の cargo で代用した
