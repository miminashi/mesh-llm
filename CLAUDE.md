# CLAUDE.md

リポジトリ全体の開発ルールは [AGENTS.md](AGENTS.md) に従う。本ファイルでは、aws-v100 でのビルド・起動手順と、Claude Code による作業レポートの執筆ルールを定める。

## aws-v100 でのビルドと起動

aws-v100（Tesla V100-SXM2 16GB、CUDA sm_70）で、このリポジトリを CUDA バックエンドでビルドし、mesh-llm を split モードで動かしている。GPU サーバの排他ロックは llm-server-ops の `gpu-server` スキル（`lock.sh aws-v100` / `unlock.sh aws-v100`）で取る。

### 前提（2026-10-27 時点）

- ソースは `~/src/mesh-llm`。v100 上の main はワークステーションの main（フォーク `miminashi/mesh-llm` に upstream をマージしたもの）と同じにしてあり、v100 で `git pull` はしない。更新はワークステーションから送る:

  ```bash
  # ワークステーション側
  git fetch upstream && git merge --no-edit upstream/main
  git push ssh://aws-v100/home/ubuntu/src/mesh-llm main:refs/heads/ws-main
  ssh aws-v100 'cd ~/src/mesh-llm && git merge --ff-only ws-main'
  ```

- ツールはユーザー領域に導入済み: rustup（stable 1.99.1）、just、sccache 0.16.0（`~/.cargo/bin`）、Node 24（`~/.local/bin/node`）、pnpm（`~/.local/share/pnpm`）
- apt で導入済み: `patchelf`（ネイティブランタイムのパッケージングに必須）、mold、lld（`just bootstrap-build-tools`）
- PATH などは `~/mesh-env.sh` にまとめてあり、ビルド・起動の前に必ず読み込む:

  ```bash
  export PATH=$HOME/.local/share/pnpm/bin:$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/cuda-12.9/bin:$PATH
  export CUDA_HOME=/usr/local/cuda-12.9
  export CARGO_INCREMENTAL=0
  ```

- `sudo` は `sudo -n` で通る

### ビルド

tmux セッション `mesh-build` で実行し、ログを `~/mesh-build.log` に残す:

```bash
ssh aws-v100
tmux kill-session -t mesh-build 2>/dev/null   # 前回のセッションが残っていると new-session が duplicate session で失敗する
tmux new-session -d -s mesh-build -c ~/src/mesh-llm \
  "bash -c '. ~/mesh-env.sh; just build cuda 70 2>&1 | tee ~/mesh-build.log; echo EXIT=\${PIPESTATUS[0]} | tee -a ~/mesh-build.log; exec bash'"
```

- 完了判定: `grep '^EXIT=' ~/mesh-build.log` が `EXIT=0` なら成功。進捗は `tail ~/mesh-build.log` か `tmux attach -t mesh-build` で見る
- **ビルド中も mesh-llm（セッション `mesh`）は止めない**。`EXIT=0` を確認してから、下の「起動し直し」の手順で新しいバイナリに切り替える。失敗した場合は古いバイナリのまま動かし続けられる

- **just の引数は位置指定で渡す**（`just build cuda 70`）。`just build backend=cuda cuda_arch=70` と書くと `--backend "backend=cuda"` という文字列がそのまま渡ってしまう
- 成果物: `target/debug/mesh-llm`（ホスト）と `target/debug/native-runtimes`（Skippy の CUDA ランタイム）
- llama.cpp の pin が変わると CUDA カーネルのビルドがやり直しになり、時間がかかる
- **ディスク容量に注意**（ルート FS は 116G）。2026-10-27 の再ビルドでは `target/debug/incremental` が 12G に膨らんで満杯になった。`CARGO_INCREMENTAL=0` はその対策。ビルド前に `df -h /` で 10G 以上の空きを確認し、足りなければ `target/debug/incremental` や古い `.deps/llama-build/build-stage-*` を消す

### 起動

tmux セッション `mesh` で起動する（ユーザーが TUI を見るため。`ssh -t aws-v100 tmux attach -t mesh` で表示、Ctrl-b d で抜ける）:

```bash
M=~/models/huihui-qwen3.8-27b
tmux new-session -d -s mesh -x 200 -y 50 -c ~/src/mesh-llm \
  "bash -c '. ~/mesh-env.sh; ./target/debug/mesh-llm serve \
    --gguf $M/Huihui-Qwen3.8-27B-abliterated-UD-Q4_K_XL.gguf \
    --mmproj $M/mmproj-model-bf16.gguf \
    --split --bind-port 17777 --listen-all; echo EXIT=\$?; exec bash'"
```

| オプション | 意味 |
|---|---|
| `--gguf` / `--mmproj` | Huihui-Qwen3.8-27B Q4_K_XL（17.4GB）と画像入力用プロジェクタ |
| `--split` | 1 台に収まるかに関係なく層を複数ノードに分割する。ピアが来るまで `standby` のまま |
| `--bind-port 17777` | ピア間 QUIC を UDP 17777 に固定する |
| `--listen-all` | API（9337）とコンソール（3131）を `0.0.0.0` で待ち受ける |

- 停止: TUI で `q`（`tmux send-keys -t mesh q`）してから `tmux kill-session -t mesh`
- 起動し直し（ビルド後の切り替え）: 停止 → プロセスが消えたのを確認 → 上の起動コマンド、の順に行う。`mesh` セッションが残ったままだと new-session は失敗する

  ```bash
  tmux send-keys -t mesh q
  while ps -eo args | grep -q '^./target/debug/mesh-llm serve'; do sleep 1; done
  tmux kill-session -t mesh
  # → 上の起動コマンドを実行
  curl -s http://10.22.5.2:3131/api/status | jq -r .version   # 新しいコミットの SHA が付いていることを確認
  ```

  `pgrep -f` / `pkill -f` は ssh 越しに実行するとコマンドライン文字列自身にマッチしてシェルを巻き込むので使わない
- 招待トークン: `curl -s http://10.22.5.2:3131/api/status | jq -r .token`
- ufw: 9337/tcp と 3131/tcp は `10.0.0.0/8` から、17777/udp は Anywhere から許可済み

### 注意事項

- v100 には BMC が無く、固まると現地でしか復旧できない。再起動は行わない（apt がカーネル更新による再起動を推奨しているが保留中）
- `nvidia-smi` で見える GPU は 1 枚（16GB）だけ。2 枚目は PCIe バスから消えている
- mesh を作るときに `--require-release-attestation` を付けない。設定ファイルの `mesh_requirements.require_release_attestation` も true にしない（既定値は false）。手元ビルドには release の署名が無いので、true にするとメンバーのノードどうしが参加を拒否される
- この fork では release を行わないので、`release.yml` と `resume-crates-release.yml` は `gh workflow disable` で無効にしてある（issue #3。戻すときは `gh workflow enable <file> --repo miminashi/mesh-llm`）
- `agentic-replay-nightly.yml`（Goose による自動修復を含む nightly）も `gh workflow disable` で無効にしてある（issue #21。元々 `github.repository == 'Mesh-LLM/mesh-llm'` の条件で fork では skip され、self-hosted runner も secret も無いが、明示的に止めた）
- 配布用アーカイブを作る `just bundle` / `just release-bundle`（中身は `scripts/package-release.sh`）は使わない。直接の経路では native runtime の checksum を照合せずに digest を付けてしまう（issue #20、未修正で close）。使う必要が出たら、先に #20 を開き直して直す

## レポート執筆ルール

### レポート作成タイミング

plan mode で計画を立ててまとまった作業を行った場合、完了時に必ずレポートを作成する。

- plan mode で作業計画を立てる際、レポート作成を作業内容に含めること
- ユーザから明示的に「レポート不要」と指示された場合を除く

### 保存先

- `report/` ディレクトリ直下（リポジトリルート基準）
- ワークツリーで作業している場合も、レポートは常にメインチェックアウト（`/home/ubuntu/projects/mesh-llm/report/`）に作成する

### ファイル名

- 形式: `yyyy-mm-dd_hhmmss_レポート名.md`
- レポート名（ファイル名部分）は英語で記載する（識別子・キーワードによる grep 性を高める）
- タイムスタンプは `TZ=Asia/Tokyo date +%Y-%m-%d_%H%M%S` コマンドで取得する（LLM が時刻を推測してはならない。システムが UTC の場合でも JST で取得される）

### タイトル (H1)

- 平易な日本語で 50 字以内
- サーバ名・コミット SHA・PR 番号等の識別子は最小限、日時はタイトルに入れない（ファイル名側で吸収）
- 発見の詳細・数値・ハイライトはタイトルに詰め込まず、本文の「核心発見サマリ」に書く（訂正のたびにタイトルを直す必要が生じるため）
- 必要なら em dash (`—`) で結論を一行付ける形式が使える（例: `# 2 ノード分割推論のレイテンシ計測 — RPC 往復が支配的と判明`）

### メタ情報

タイトルの直後に、以下 3 行を bullet で置く：

```markdown
- **実施日時**: 2026年10月27日 13:22 〜 13:55 JST (2 ノード構成での分割推論ベンチマーク)
- **報告日時**: 2026年10月27日 13:55 JST
- **作成者**: Claude Opus 5.5
```

- 日時は JST で記載する（システムが UTC の場合は +9 時間に変換）
- 「実施日時」の末尾に括弧書きで **その時間で何をしたか** の要約を付ける
- 「作成者」には、レポートを書いた Claude のモデル名を **`Claude` 接頭辞付きで** 記載する（例: `Claude Opus 5.5` / `Claude Fable 5.1` / `Claude Sonnet 5.5` / `Claude Haiku 5.5`）。モデル名はシステムプロンプトの「You are powered by the model named …」で確認できる（そこには `Claude` 接頭辞が付いていないため、記載時は補って書く）。常にそのセッションで動作中のモデル名を記録すること

### 概要 (必須)

タイトルとメタ情報の直下に、他のいかなるセクションよりも先に `## 概要` を置く。

- **通読できる平易な日本語で書く**（段落として読める文章、箇条書きの羅列にしない）
- 5〜8 段落を目安
- **細かい事実（数値、識別子、パス、コミット SHA、PR 番号、統計値、試行回数、対象 LLM のモデル名、ハッシュ等）は概要から省いてよい**（メタ情報の作成者フィールドに書く Claude のモデル名とは別物）。各事実は他のセクション（環境情報 / 結果詳細 / 核心発見サマリ / 参照レポート 等）に書き、概要は「何のため」「何をやった」「何が分かった」「次にやる」を平易な日本語で読める形にまとめる
- 位置づけは **「ユーザー（人間）が手短にレポート全容を把握するためのもの」** — 技術的アブスト（結論数値・統計値等）は「核心発見サマリ」で別途書く

### セクション一覧 (カノニカル名)

以下のセクションを設ける。表記揺れを避け、下記のカノニカル名を使用する：

| カノニカル名 | 要否 | 用途 |
|---|---|---|
| `## 概要` | **必須** | ユーザー（人間）が手短にレポート全容を把握できる、通読可能な平易日本語（詳細は前節） |
| `## 添付ファイル` | 実質必須 (plan.md 添付のため) | 生ログ・スクリーンショット・スクリプト等へのリンク |
| `## 核心発見サマリ` | 任意 (実測系では推奨) | 数値・PNG・統計値等を凝縮した技術者向けアブスト |
| `## 前提・目的` | 任意 | 背景、目的、前提条件 |
| `## 環境情報` | 任意 (実測系では推奨) | サーバ構成、GPU、モデル、ブランチ・コミット、バージョン等 |
| `## 参照レポート` | 任意 | 過去のレポートへの相対リンク |
| `## 再現方法` | 任意 | 手順・コマンド |
| `## 結果詳細` | 任意 | 実験結果・観測データ |
| `## 副次発見` | 任意 | 副次的観測・気づき |
| `## 残課題` | 任意 | 次セッションへの引き継ぎ |
| `## 結論・対応` | 任意 (核心発見サマリで足りるなら省略可) | 結論、対応方針 |

`## 概要` を H1 とメタ情報の直下（他セクションより先）に置くこと以外、セクション順序は必須ではない。レポート内容に応じて自然な並びを選んでよい。

### 核心発見サマリ の書式

「核心発見サマリ」は技術者向けアブスト（結論数値、統計検定の p 値、計測値等を凝縮）。

- **実測データを持つレポートでは、同セクション冒頭に PNG を画像埋め込み `![...](...)` する**（リンクのみは不可）
- 位置は `## 核心発見サマリ` の直後・空行を挟んで最初のコンテンツ
- PNG は matplotlib 等で生成し、時系列プロットの場合は回帰直線を重畳する
- Alt テキストは図の一行説明を日本語で
- PNG 直後に `**結論**:` で始まる 1 段落テキストを続ける

コード変更のみのレポート（実測データを持たない）では PNG 埋め込みは不要。

### 添付ファイル

- スクリーンショット・ログファイル・スクリプト・データ等は `report/attachment/<レポートファイル名>/` ディレクトリに格納する
  - `<レポートファイル名>` は `.md` を除いたファイル名（例: `2026-10-27_135433_split_inference_latency`）
- レポート本文から相対パスでリンクする（例: `![screenshot](attachment/2026-10-27_135433_split_inference_latency/screenshot.png)`）— **添付ファイルディレクトリ配下 (`attachment/`) へのリンクでは `./` プレフィックスを付けない**（参照レポートセクションから他 `.md` にリンクする場合は `./yyyy-...md` 表記を使う）

#### プランファイルの添付（必須）

plan mode で作業を行った場合、以下の手順でプランファイルを添付する：

1. 添付ディレクトリを作成: `mkdir -p report/attachment/<レポートファイル名>/`
2. プランファイルをコピー: `cp /home/ubuntu/.claude/plans/<plan-name>.md report/attachment/<レポートファイル名>/plan.md`
3. `## 添付ファイル` セクションから `[実装プラン](attachment/<レポートファイル名>/plan.md)` のようにリンク

### フォーマット例

````markdown
# 2 ノード分割推論のレイテンシ計測 — RPC 往復が支配的と判明

- **実施日時**: 2026年10月27日 10:05 〜 12:40 JST (2 ノード構成での分割推論ベンチマークとプロファイル取得)
- **報告日時**: 2026年10月27日 13:10 JST
- **作成者**: Claude Opus 5.5

## 概要

（何のために、何をやり、何が分かり、次に何をするかを、5〜8 段落の平易な日本語で書く）

## 添付ファイル

- [実装プラン](attachment/2026-10-27_131005_split_inference_latency/plan.md)
- [ベンチマーク生ログ](attachment/2026-10-27_131005_split_inference_latency/bench.log)

## 核心発見サマリ

![トークンあたりレイテンシの内訳（計算 vs RPC 往復）](attachment/2026-10-27_131005_split_inference_latency/summary.png)

**結論**: （結論数値・統計値等を凝縮した 1 段落）

## 前提・目的

- **背景**: …
- **目的**: …
- **前提条件**: …

## 環境情報

- ノード: …
- GPU: …
- ブランチ / コミット: …

## 再現方法

（手順・コマンドを記載）

## 参照レポート

- [前回のレイテンシ計測](./2026-10-20_093000_single_node_latency_baseline.md)

## 結果詳細

（計測結果・観測データ）

## 副次発見

（副次的な観測）

## 残課題

- （次セッションへの引き継ぎ）
````
