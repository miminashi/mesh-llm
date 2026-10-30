# issue #1: 上流修正の検証と close

## Context

issue #1（remote 経路の課金判定が公開 model ID の価格を見落とす）は、当初「上流で直るまで `payments` を既定 feature から外し、直ったら戻して close」という方針だった。
再調査で、上流が既に修正済みであり、fork の main にも取り込まれていることが分かった:

- 上流 issue Mesh-LLM/mesh-llm#2246（同じ問題）→ 2026-10-29 close (COMPLETED)
- 上流 PR #2598「fix(runtime): enforce seller pricing under a model's public alias」→ `fix/megabugs` へマージ → PR #2614（`b8b0da343`, 2026-10-29）で upstream main へ
- fork main に `b8b0da343` が含まれる（issue 作成時点の `286bd1aea` より後）
- 修正内容: `response/payment_admission.rs::requires_payment` が、書き換え後の実行名と、descriptor から求めた公開 ID（profile 付き／無し）の両方で価格表を引く。`response/routing.rs:50` の seller 判定がこれを呼ぶ → issue の「期待する動作」と一致

ユーザー判断: payments を外す変更は行わず、検証してから issue を close する。

## 手順

1. **コード確認**（済、記録のみ）: `routing.rs:50` が `payment_admission::requires_payment` を使い、旧 `priced_model_requires_payment` の完全一致のみの判定が残っていないこと
2. **テスト実行**（ワークステーション）:
   ```bash
   just with-lld cargo test -p mesh-llm-host-runtime --lib payment_admission
   ```
   `payment_admission.rs` の tests（公開 alias の書き換え後でも支払いを要求する等）が通ることを確認。ログを添付用に保存
   - ビルドが native ABI 等で通らない場合は、その旨を記録しコード読みの根拠で close 判断をユーザーに確認
3. **レポート作成**: `report/<JST timestamp>_issue1_upstream_payment_alias_fix.md`（CLAUDE.md のルールに従う。plan.md とテストログを `report/attachment/<名前>/` に添付。実測グラフ不要の調査レポート）
4. **レポートをコミット・push**（main 直接 or ブランチは既存の運用に合わせる。Co-Authored-By トレーラーは付けない＝メモリのルール）
5. **issue #1 にコメントして close**: 上流 issue #2246 / PR #2598 / #2614 のリンク、fork main に含まれるコミット、テスト結果、レポートへのリンク、「payments は既定のまま（外す変更は不要になった）」を記載し `gh issue close 1 --reason completed`
6. （補足としてコメントに記載）aws-v100 で動いている mesh-llm が `b8b0da343` 以降のビルドか `/api/status` の version で確認できれば記載。ただし課金未設定なので影響なし

## 変更しないもの

- `mesh/crates/mesh-llm/Cargo.toml`, `mesh/crates/mesh-llm-host-runtime/Cargo.toml`, `mesh/scripts/build-host.sh` の `payments`

## 検証

- 手順 2 のテストが pass
- `gh issue view 1 --repo miminashi/mesh-llm` で CLOSED とコメントを確認
