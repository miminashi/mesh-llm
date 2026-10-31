## 噛み砕いた解説

### そもそも何をする処理か

`package-release.sh` は、配布用のアーカイブ（bundle）を組み立てるスクリプトです。bundle には大きく 2 つの部品が入ります。

1. **host**: `mesh-llm` 本体のバイナリ
2. **native runtime**: 推論を実際に行う CUDA などのライブラリ群（Skippy のランタイム）

native runtime は別のビルドで作られ、`manifest.json` と一緒に置かれています。この manifest には「中に入っている各ファイルの checksum（SHA-256）」が書かれています。いわば **「この箱にはこの中身が入っています」という封印付きの納品書** です。

組み立ての最後に、bundle 全体の product manifest を作ります。ここには host と runtime の digest（ハッシュ値）が記録され、利用者のノードは後でこの digest を見て「改ざんされていない正しい product か」を判断します。

### 2 つの経路

bundle の組み立て方には 2 通りあります。

| 経路 | 流れ | runtime の検査 |
|---|---|---|
| precomposed（組み立て済みを受け取る） | 受け取った runtime を `verify-native-runtime-package.sh` で検査 → `product compose --check` | **あり** |
| 直接（その場で組み立てる） | `native select-runtime` で runtime を選ぶ → `cp -R` でコピー → `product compose` で digest を作る | **なし** |

直接の経路で使われる部品は、どれも納品書と中身を突き合わせません。

- `select-runtime` は OS・アーキテクチャ・backend・CUDA のメジャーバージョンが合う runtime を **選ぶだけ** です
- `product compose` は、今ディスクにあるファイルから `tree_sha256` で **新しくハッシュを計算するだけ** です（`compose.rs:98-99`）。runtime の manifest に書かれた checksum とは比べません

### 何がまずいのか

たとえ話にすると、こうなります。

> 工場から「中身はこれ」という納品書付きの箱が届いた。precomposed の経路では、納品書と中身を照合してから出荷ラベルを貼る。ところが直接の経路では、照合せずに **今の中身をそのまま撮影して、その写真を正式な出荷ラベルにしてしまう**。

このため、`target/.../native-runtimes` 内の `.so` などが

- ビルド後にディスクの不具合や途中で止まったコピーで壊れていた
- 誰か（または何か）によって書き換えられていた

という状態でも、スクリプトはエラーにならず、**壊れた中身に合う「正しい」digest を付けて** bundle を作ってしまいます。受け取った側のノードが digest を確かめても一致するので、異常に気づけません。本来なら「納品書と中身が違う」と止められたはずのものが、正規品として通ってしまうのが問題の核心です。

fork 側の issue（jimoto-no-llm/mesh-llm#475）では、ライブラリだけを書き換えた場合に verifier は checksum mismatch で exit 1 になる一方で、`select-runtime` と compose はそのまま受け入れて新しい hash を作ることが確認されています。

### この repo での影響範囲

- この fork では release workflow を無効にしているので、CI から壊れた release が出ることはありません
- 影響を受けるのは、手元で `package-release.sh` を直接の経路で動かして bundle を作り、それを配る場合だけです
- 深刻度が Medium なのは、悪用するには **ビルドするマシンの runtime ファイルを書き換えられること** が前提になるためです。ただし、そうなった場合でも検査が素通りになるので、「最後の砦」が一つ抜けていることになります

### 直し方の方針

precomposed の経路と同じく、既存の verifier を 1 行呼ぶだけで足ります。

```bash
runtime_dir="$(select_native_runtime_dir)"
"$SCRIPT_DIR/verify-native-runtime-package.sh" "$runtime_dir"   # ← 追加：コピー前に manifest の checksum と照合
bundled_runtime="$bundle_dir/native-runtimes/$(basename "$runtime_dir")"
mkdir -p "$(dirname "$bundled_runtime")"
cp -R "$runtime_dir" "$bundled_runtime"
write_product_manifest ...
```

- **コピーの前** に検査するので、checksum が合わなければ bundle に何も入れずに止まります（`set -e` で exit）
- 正しい runtime であれば検査を通るだけなので、今の動作は変わりません
- 念を入れるなら、コピー後の `$bundled_runtime` も検査すれば、コピーの途中で壊れた場合まで検出できます
- テストとしては「runtime の `.so` を 1 バイト書き換えてから直接の経路を走らせ、product manifest が作られずに失敗すること」を確認すればよいです

