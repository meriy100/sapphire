# parse-snapshot

I4 パーサ（`crates/sapphire-compiler/src/parser/`）が M9 例題プロ
グラム（`docs/spec/12-example-programs.md` / `examples/sources/`）
をどう受理するかを確認するための **AST スナップショット** 集。

仕様：

- `examples/sources/<name>/*.sp` を `parse_dump` 実行例に通して出
  力した `{:#?}` 形式の AST をそのまま保存している。
- 入力は I3 のレキサ → I4 のレイアウト解決パス → I4 のパーサを一
  気通貫に通したもの。`sapphire_compiler::parser::parse(&str)` が
  実体。

## ファイル対応

| 入力 `.sp`                                              | スナップショット                                     |
|---------------------------------------------------------|------------------------------------------------------|
| `examples/sources/01-hello-ruby/Main.sp`                | `01-hello-ruby-Main.ast.txt`                         |
| `examples/sources/02-parse-numbers/NumberSum.sp`        | `02-parse-numbers-NumberSum.ast.txt`                 |
| `examples/sources/03-students-records/Students.sp`      | `03-students-records-Students.ast.txt`               |
| `examples/sources/04-fetch-summarise/Fetch.sp`          | `04-fetch-summarise-Fetch.ast.txt`                   |
| `examples/sources/04-fetch-summarise/Http.sp`           | `04-fetch-summarise-Http.ast.txt`                    |

## 再生成

リポジトリルートから以下を流し、差分がなければスナップショット
は最新である。

```
for src in \
  examples/sources/01-hello-ruby/Main.sp \
  examples/sources/02-parse-numbers/NumberSum.sp \
  examples/sources/03-students-records/Students.sp \
  examples/sources/04-fetch-summarise/Fetch.sp \
  examples/sources/04-fetch-summarise/Http.sp
do
  out="examples/parse-snapshot/$(basename "$(dirname "$src")")-$(basename "$src" .sp).ast.txt"
  cargo run -q -p sapphire-compiler --example parse_dump -- "$src" > "$out"
done
```

差分を確認したい場合：

```
cargo run -q -p sapphire-compiler --example parse_dump -- \
    examples/sources/01-hello-ruby/Main.sp \
  | diff -u examples/parse-snapshot/01-hello-ruby-Main.ast.txt -
```

## 出力の読み方

1 行に 1 ノード、`Debug` の `#?` 整形。`Module` 以下に
`ModuleHeader` → `imports` → `decls` が並ぶ。各ノードは
`sapphire_core::ast` の型と一致しており、`Span { start, end }` は
元ソースへのバイトオフセット（半開区間）。レイアウトが挿入した
仮想 `{` / `;` / `}` は AST に現れない（パーサ側で既に吸収して
いる）。

## 含まれないもの

- **名前解決**。`Var` の `module: None` は「不明な修飾」ではなく
  「明示的な修飾なし」の意味。`Foo.bar` のような修飾参照は
  `module: Some(ModName { segments: ["Foo"], ... })` を伴う。
- **型検査**。`Scheme` / `Type` の構造は構文上の形のままで、kind
  整合や主要型の推論は未実施。
- **糖衣展開**。`[x, y, z]` は `Expr::ListLit` のまま、`if` は
  `Expr::If` のまま。`case` への脱糖は elaboration 層で行う。

これらは I5（name resolution）/ I6（type checker）以降の仕事。
