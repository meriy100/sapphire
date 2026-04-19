# typecheck-snapshot

I6 型検査器の出力を、spec 12 の 4 例題に対して固めたスナップショッ
トを置く。

## 生成手順

ワークスペースルートから次を実行する。

```bash
cargo run -p sapphire-compiler --example typecheck_dump -- \
  examples/sources/01-hello-ruby/Main.sp \
  > examples/typecheck-snapshot/01-hello-ruby.types.txt

cargo run -p sapphire-compiler --example typecheck_dump -- \
  examples/sources/02-parse-numbers/NumberSum.sp \
  > examples/typecheck-snapshot/02-parse-numbers.types.txt

cargo run -p sapphire-compiler --example typecheck_dump -- \
  examples/sources/03-students-records/Students.sp \
  > examples/typecheck-snapshot/03-students-records.types.txt

cargo run -p sapphire-compiler --example typecheck_dump -- \
  examples/sources/04-fetch-summarise/ \
  > examples/typecheck-snapshot/04-fetch-summarise.types.txt
```

複数ファイルを持つ 04 では、ディレクトリ指定で resolver がトポ
ロジカル順に解決する。

## 期待値の見方

各ファイルは次の順で出力される：

- `TypedProgram (N modules)` ヘッダ。
- モジュールごとに `== module <name> ==`。
- 各 top-level value binding について `  <name> : <scheme>`。

Scheme は `forall tv1 tv2. (Constraint1, Constraint2) => Ty` 形式。
シグニチャが書かれているものはシグニチャのスキームがそのまま出る。
書かれていないものは推論結果 + 一般化が出る。

## 何を確認するか

- **01 hello-ruby**: `main : Ruby {}` + `rubyPuts` など Ruby monad
  の基本形が出ること。`do` 展開 + `Monad Ruby` instance が resolve
  されている。
- **02 parse-numbers**: `parseAll : List String -> Result String (List Int)`
  が出ること。`Monad (Result e)` instance 経由で `do` が通ってい
  る。
- **03 students-records**: record 型の署名が record literal の field
  セットとして現れること。`foldr1` は署名ありで `forall a. ...` に
  なる。`topScorersByGrade` 等、record update / field access が全て
  通っていること。
- **04 fetch-summarise**: モジュール跨ぎ ADT (`HttpError`) の参照が
  解決し、`get : String -> Ruby (Result HttpError String)` になる
  こと。

実行例の `Ruby {  }` の二重スペースは現在の Display 形式に由来す
る軽微な余剰で、後続 commit で整える候補。
