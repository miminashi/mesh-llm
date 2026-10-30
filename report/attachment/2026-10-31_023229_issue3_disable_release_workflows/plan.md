# issue #3 を「fork では対象外」として閉じ、release workflow を止める

## Context

issue #3（attestation が無効でも composer / host producer を通る）は、`release.yml` の release の中でしか起きない。この fork は限られたメンバーだけで使い、広く release しない。upstream に修正を送る予定もない。そこで、コードは変えずに release の workflow を止め、issue を not planned で閉じる。あわせて、署名の無い手元ビルドどうしの mesh 参加が拒否されないように、注意点を CLAUDE.md に書く。

確認済みの事実:
- `release.yml` / `resume-crates-release.yml` は fork で `active`。実行履歴は無い（`workflow_dispatch` だけ）
- 参加の条件として attestation を必須にする設定は、mesh を作るときの CLI フラグ `--require-release-attestation`（`mesh-llm-cli/src/parser/top_level_options.rs:201-203`）か、設定ファイルの `mesh_requirements.require_release_attestation`（`mesh-llm-config/src/model.rs:314`、既定値は false）
- v100 の `~/.mesh-llm` にこの設定は無く、起動コマンドにもこのフラグは無い

## 手順

1. release の workflow を無効にする
   - `gh workflow disable release.yml --repo miminashi/mesh-llm`
   - `gh workflow disable resume-crates-release.yml --repo miminashi/mesh-llm`
2. issue #3 にコメントを書き、not planned で閉じる
   - コメントの内容: この fork では release を行わないため対象外とすること。上の 2 つの workflow を無効にしたこと（元に戻すときは `gh workflow enable`）。コードの attestation は upstream の取り込みと mesh 参加の判定に関わるので、残したこと。mesh を作るときに `--require-release-attestation` を使わないという注意点を、CLAUDE.md に書いたこと
   - `gh issue close 3 -R miminashi/mesh-llm --reason "not planned"`
3. `CLAUDE.md` の「aws-v100 でのビルドと起動 > 注意事項」に、次の bullet を足す
   - mesh を作るときに `--require-release-attestation` を付けない。設定ファイルの `mesh_requirements.require_release_attestation` も true にしない。手元ビルドには release の署名が無いので、true にするとメンバーのノードどうしが参加を拒否される（issue #3 で fork の release を止めた経緯）
   - fork では `release.yml` と `resume-crates-release.yml` を手動で無効にしてある（`gh workflow enable` で戻せる）
   - CLAUDE.md は未追跡のファイルなので、コミットはしない
4. レポートを書く（CLAUDE.md のルールに従う）
   - `report/<JST タイムスタンプ>_issue3_disable_release_workflows.md`
   - 内容: upstream の対応状況の再調査、PREVERIFIED で確認が飛ばされることの発見、issue へのコメント 2 件、判断と対応
   - このプランを `report/attachment/<名前>/plan.md` に添付する
   - 前回までと同じように `docs(report): ...` でコミットし、origin/main に push する（Co-Authored-By は付けない）

## 確認

- `gh workflow list -R miminashi/mesh-llm --all` で、2 つの workflow が `disabled_manually` になっている
- `gh issue view 3 -R miminashi/mesh-llm --json state,stateReason` が `CLOSED` / `NOT_PLANNED`
- `git status` で、レポートがコミット済みで、CLAUDE.md だけが未追跡のまま
