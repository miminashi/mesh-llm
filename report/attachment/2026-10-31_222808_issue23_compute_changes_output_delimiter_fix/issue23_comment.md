## 噛み砕いた解説

### そもそも何をしている処理か

CI では最初に `compute-changes` という action が動き、「この PR（または push）でどのファイルが変わったか」を調べます。その結果を見て、後のジョブが「Rust のテストを回すか」「UI だけの変更か」などを決めます。

action の結果を後の step に渡すには、`$GITHUB_OUTPUT` というファイルに `名前=値` の形で 1 行ずつ書きます。値が複数行になるとき（変更ファイルの一覧など）は、次のような「ヒアドキュメント風」の書き方をします。

```
changed_files<<EOF
src/a.rs
src/b.rs
EOF
affected_crates=[...]
all_rust=true
...
```

`<<EOF` から、`EOF` だけが書かれた行までが 1 つの値です。`EOF` という文字列は「ここで値が終わり」という合図にすぎず、何でも構いません。

### 何が問題か

`derive-outputs.sh` は、この合図に毎回同じ `EOF` という文字列を使い、その間に `git diff --name-only` の結果をそのまま流し込んでいます。

ここで、PR に **`EOF` という名前のファイル**（repo のルート直下）が含まれていると、一覧の途中に `EOF` だけの行が現れます。GitHub はそこで「値が終わった」と判断するので、それ以降のファイル名は変更一覧ではなく、`名前=値` の命令として読まれてしまいます。

```
changed_files<<EOF
EOF                          ← ファイル名なのに「値の終わり」と解釈される
all_rust=false               ← ファイル名なのに「all_rust を false にせよ」と解釈される
src/normal.rs                ← 「名前=値」の形でないので形式エラー
EOF                          ← スクリプト本来の終わりの合図。これも宙に浮く
affected_crates=[...]
...
```

文字列を「データ」として流しているつもりが、区切り文字と同じ中身が来ると「命令」に化ける、という点で SQL インジェクションやシェルのクォート漏れと同じ種類のバグです。

### 何が起き得るか

1. **CI が壊れる（確実に起きる側）**: `EOF` という名前のファイルがあるだけで、`名前=値` の形でない行が出てくるので、step は形式エラーで落ちるはずです。悪意が無くても、たまたまそういうファイルを置けば起きます。変更一覧もそこで途切れます。
2. **output の偽造（理屈の上では起き得る側）**: ファイル名を `EOF`、`xxx=yyy`、`zzz<<EOF` のように細工して並べれば、エラーを避けつつ好きな名前の output を書き込める可能性があります。ただし、スクリプトは後で `affected_crates=` などを改めて書くので、同じ名前なら後の正しい値で上書きされると思われます。実際に何を偽造できるかは確かめていません。

### この fork での実際の危険度

- `compute-changes` を使う PR 用 workflow（`pr_linux.yml`、`pr_macos.yml` など）は `pull_request` トリガーで動きます。この場合 workflow と action のコードは PR 側のものが使われ、fork からの PR には secret も書き込み権限も渡りません。PR を出す人はもともと `derive-outputs.sh` 自体を書き換えられるので、このバグで新しく得られる権限はほぼありません。
- `ci-pr-canary-lane.yml` は PR の merge commit に対してこの action を動かしますが、呼び出し元の `pr_ci_canary.yml` は upstream（`Mesh-LLM/mesh-llm`）の `@main` を参照しています。
- この fork では他人の PR を受け付ける運用ではないので、現実の影響は 1. の「CI が落ちる」程度と見ています。Medium というより、壊れやすさを直す修正に近いものです。

### 直し方

GitHub Docs が勧めているとおり、区切りを毎回ランダムに作れば、ファイル名と衝突しません。

```bash
delim="EOF_$(openssl rand -hex 16)"
{
  echo "changed_files<<$delim"
  grep -v '^__force_all__$' /tmp/changed_files.txt || true
  echo "$delim"
  ...
}
```

もう 1 つの案は、`changed_files` の output をやめることです。後段の `plan-ci` はすでに `/tmp/changed_files.txt` を直接読んでいるので、ファイルで渡す形に揃えられるかもしれません。ただ、`ci-pr-canary-lane.yml` が `steps.changes.outputs.changed_files` を参照しているので、そこも一緒に直す必要があります。

修正後は、`EOF` と普通のファイル（例: `normal.rs`）を両方含む変更で step が成功し、`changed_files` に両方とも入っていることを確かめれば十分です。
