# bytes 取得でサーバ申告サイズ分を先に確保しないよう修正

- **実施日時**: 2026年11月1日 17:44 〜 17:58 JST (issue #18 の修正、テスト追加、修正前の挙動の再現)
- **報告日時**: 2026年11月1日 17:58 JST
- **作成者**: Claude Opus 5.5

## 概要

Hugging Face Hub からファイルをメモリ上に読み込む関数は、受信を始める前に、サーバが応答ヘッダで申告したサイズの分だけメモリを確保していました。届く量にも上限がありませんでした。そのため、とても大きなサイズを申告する応答を受けると、中身を 1 byte も読む前にプロセスが落ちていました。issue #18 は、これを直すためのものです。

修正では、申告されたサイズを目安としてだけ使うようにしました。先に確保するのは最大 8 MiB までで、それより大きいファイルは、実際に届いた分に合わせてメモリを増やします。あわせて、呼び出し側が「最大何 byte まで受け取るか」を指定できるようにしました。上限を超えると分かった時点で、専用のエラーを返します。申告の時点で超えていれば本体を読む前に、申告が無くても届いた量が超えたところで止まります。

この関数を今使っているのは、モデルの catalog JSON を読む運用ツールだけです。そこには 16 MiB の上限を付けました。上限を指定しない呼び出しは今までどおり動くので、既存の利用者に影響はありません。

ローカルに立てた試験用のサーバでテストを 6 件追加しました。修正を外して同じテストを動かすと、巨大なサイズを申告されただけでテストのプロセスが SIGABRT で落ちました。issue に書かれていた「プロセスが落ちる」が実際に起きることを確かめたうえで、修正後はすべて通ることを確認しました。

取り込み元からの変更を記録する `SOURCE_PROVENANCE.json` にも、今回の変更を追記しました。upstream と jimoto-no-llm には、この件の修正提案はまだありません（同日に確認済み）。upstream が同じ箇所を直したら、マージのときに upstream 側を採用します。

## 添付ファイル

- [skippy-hf-hub のテスト結果（修正後）](attachment/2026-11-01_175353_issue18_hf_hub_bytes_download_limit_fix/skippy-hf-hub-test.log)

plan mode を使っていないので、プランファイルはありません。

## 核心発見サマリ

**結論**: `download_file_to_bytes_impl` は `X-Linked-Size`／`Content-Length` の値をそのまま `BytesMut::with_capacity` に渡していた。`X-Linked-Size: 4611686018427387904`（2^62）を返す応答では、修正前の実装でテストプロセスが SIGABRT になった（追加した 6 件のうち 4 件が abort／失敗）。修正後は、先に確保する量を `min(期待長, max_bytes, 8 MiB)` に抑え、`max_bytes` を超えると `HFError::ResponseTooLarge` を返す。`skippy-hf-hub` の全テスト（lib 222 件、`tests/` 2 件、doctest 21 件）と、`skippy-hf-hub`・`skippy-model-package` の `clippy --all-targets -D warnings` が通った。

## 前提・目的

- **背景**: issue #18（元は jimoto-no-llm/mesh-llm#361）。`skippy-hf-hub` は Mesh-LLM/hf-hub から取り込んだ crate
- **目的**: サイズのヘッダが大きいだけで巨大な確保をしないこと。用途ごとに受け取る量の上限を設定でき、超えたら分かりやすいエラーになること。小さな catalog は今までどおり取得できること
- **攻撃の前提**: 接続先の応答を操れること（`HF_ENDPOINT` で信頼できないミラーを向いている、など）。被害は DoS

## 環境情報

- ワークステーション（Linux 6.8、メモリ 31GB）、rustc 1.99.1
- sccache が無いので `RUSTC_WRAPPER=` を付けて cargo を直接実行した
- GNU ld でのリンクは OOM になるので、rustup 同梱の lld（`gcc-ld`）を `PATH` の先頭に足して `-j 8` で実行した
- ブランチ: `fix/hf-hub-bytes-download-limit`（`origin/main` の `1a0f96438` から分岐）
- 変更ファイル:
  - `skippy/crates/skippy-hf-hub/src/repository/download.rs`
  - `skippy/crates/skippy-hf-hub/src/error.rs`
  - `skippy/crates/skippy-hf-hub/SOURCE_PROVENANCE.json`
  - `skippy/crates/skippy-model-package/src/bin/queue-unsloth-layer-packages.rs`

## 再現方法

```bash
cd /home/ubuntu/projects/mesh-llm-worktrees/fix-hf-hub-bytes-download-limit
export PATH=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld:$PATH
export RUSTC_WRAPPER=
cargo test -j 8 -p skippy-hf-hub --lib bytes_limit
cargo test -j 8 -p skippy-hf-hub
cargo clippy -j 8 -p skippy-hf-hub -p skippy-model-package --all-targets -- -D warnings
```

修正前の挙動は、`download_file_to_bytes_impl` の本体だけを元の実装（`content_length.unwrap_or(0) as usize` を `with_capacity` に渡し、上限なしで `extend_from_slice`）に一時的に戻して、各テストを個別に実行して確かめた。

## 結果詳細

### 変更内容

- `download_file_to_bytes`（async と blocking の両方）に `max_bytes: Option<u64>` を追加した。指定しなければ今までどおり上限なし
- 先に確保する量は `bytes_preallocation(期待長, max_bytes)` = `min(期待長, max_bytes, MAX_BYTES_PREALLOCATION = 8 MiB)`。`u64` から `usize` への変換も `try_from` で飽和させる
- 期待長は、range 指定があればその長さ、無ければ `extract_file_size` の値。range 付きのリクエストでも `X-Linked-Size` はファイル全体のサイズを返すので、range の長さを優先する
- 期待長が `max_bytes` を超えていれば、本体を読む前に `HFError::ResponseTooLarge { path, limit }` を返す。受信中も、累計が `max_bytes` を超えるチャンクが来た時点で同じエラーを返す
- `HFError` は `#[non_exhaustive]` なので、variant の追加は既存の利用者を壊さない
- `queue-unsloth-layer-packages` の catalog エントリ取得に `MAX_CATALOG_ENTRY_BYTES = 16 MiB` を付けた

### 追加したテスト（`repository::download::bytes_limit_tests`）

| テスト | 内容 | 修正前 | 修正後 |
|---|---|---|---|
| `preallocation_is_capped_regardless_of_advertised_size` | 確保量の計算（2^62、`u64::MAX`、`max_bytes` で頭打ち） | ok（新しい関数の単体テスト） | ok |
| `huge_advertised_size_does_not_preallocate` | `X-Linked-Size: 2^62` で小さな JSON を返す。上限なしでも取得できる | SIGABRT | ok |
| `advertised_size_over_max_bytes_is_rejected_before_reading` | 申告が `max_bytes` を超えたら `ResponseTooLarge` | SIGABRT | ok |
| `body_over_max_bytes_without_size_header_is_rejected` | chunked でサイズのヘッダ無し、4096 byte の本体に `max_bytes = 1000` | FAILED（全部受け取ってしまう） | ok |
| `body_within_max_bytes_is_returned` | ちょうど上限（1000 byte）の本体は受け取れる | ok | ok |
| `range_within_max_bytes_ignores_whole_file_size` | range 0..4 に `max_bytes = 16`、`X-Linked-Size` は 2^62 | SIGABRT | ok |

## 副次発見

- 修正前の実装では、2^62 byte の `with_capacity` は panic ではなく、メモリ確保の失敗による abort（SIGABRT）になった。`catch_unwind` などでは拾えず、テストハーネスごと落ちる

## 残課題

- `download_file_stream`（stream をそのまま返す API）は呼び出し側で読む量を決めるので、変更していない
- 上限を指定しない呼び出しは、今も本体のサイズに上限がない。新しく呼び出しを足すときは `max_bytes` を付ける
- upstream が同じ箇所を直したら、マージのときに upstream 側の修正を採用する
- PR マージ後に worktree を片付ける
