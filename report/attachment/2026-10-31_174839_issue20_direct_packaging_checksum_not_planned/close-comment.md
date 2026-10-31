## 対応: この fork では対象外として閉じる

この fork では配布用のアーカイブを作りません。#3 と同じく、コードは直さずに閉じます。

### `package-release.sh` を呼ぶ場所（`007d052d8` 時点）

| 呼び出し元 | 状態 |
|---|---|
| `.github/workflows/release.yml`（8 か所） | #3 で `disabled_manually` にした |
| `just bundle`（`just/mesh.just:46-52`） | 手で実行しない限り動かない。どの workflow からも呼ばれていない |
| `just release-bundle`（`just/release-bundle.just`） | 同じ |

- release 以外の workflow には、`package-release.sh` を呼ぶものはありません（`compute-changes` が、変更検知のパターンにファイル名を入れているだけです）
- aws-v100 での運用は `just build cuda 70` で `target/debug` のバイナリをそのまま起動しています。この経路は bundle を作らず、`package-release.sh` を通りません
- 問題になるのは、作った bundle を他のノードに配って digest で正規品かどうかを判断させる場合です。この fork にはそういう配布がありません

### 行ったこと

- CLAUDE.md の注意事項に、`just bundle` / `just release-bundle` を使わないことと、使う必要が出たらこの issue を開き直すことを書きました

### 将来 bundle を作るとき

release の workflow を戻すか、`just bundle` / `just release-bundle` を使う前にこの issue を開き直してください。直し方は、上の解説コメントのとおり、直接の経路で `cp -R` の前に `"$SCRIPT_DIR/verify-native-runtime-package.sh" "$runtime_dir"` を呼ぶことです。

