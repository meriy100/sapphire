# examples/lsp-hover

Sapphire LSP（L4 `textDocument/hover`）の動作確認用サンプル。
`hello.sp` を VSCode で開き、識別子の上にキャレットを置くと
Markdown ツールチップに **推論済みの型スキーム** が popup される。

サーバは `initialize` で `hover_provider = true` を宣言している。
pipeline は L5 goto-definition と共通で、`analyze` →
`resolve` → `typeck::infer::check_module` の順に回してから、
I5 の reference side table と I6 の `HashMap<String, Scheme>`
（`InferCtx.inferred`）/ `HashMap<String, CtorInfo>`
（`TypeEnv.ctors`）を引いて popup 内容を組み立てる。

## 起動手順（VSCode）

1. ワークスペースルートで LSP バイナリをビルド。

   ```
   cargo build --bin sapphire-lsp
   ```

2. VSCode 拡張の依存を入れ、TypeScript をコンパイル。

   ```
   cd editors/vscode
   npm install
   npm run compile
   ```

3. `editors/vscode` フォルダを VSCode で開いて **F5** で Extension
   Host を起動。

4. Extension Host のウィンドウで本ディレクトリの `hello.sp` を
   開く。

5. たとえば `main = do ... greet "Sapphire"` の `greet` に
   キャレットを置くと、ツールチップに以下が出る：

   ````
   ```sapphire
   greet : String -> Ruby {}
   ```
   _(top-level value)_
   ````

## 実機で動くケース（期待 hover 表示）

- **Top-level value** — `greet`, `makeMessage`, `main` など。推論
  済みのスキームが `name : τ` 形で出る。
  ```
  greet : String -> Ruby {}
  ```
  `_(top-level value)_`
- **Ruby-embedded (`:=`) binding** — `rubyPuts`。シグネチャの
  スキームが出て、タグは `_(\`:=\`-binding)_`。
- **Constructor（user-defined）** — `A`, `B`。
  ```
  A : T
  ```
  `_(constructor of \`T\`)_`
- **Constructor（prelude）** — `Just` など。
  ```
  Just : forall a. a -> Maybe a
  ```
  `_(prelude)_`
- **Prelude operator** — `++`。
  ```
  ++ : String -> String -> String
  ```
  `_(prelude)_`（演算子は `name : scheme` の name として symbol 文字
  そのまま、Haskell の section 記法との混同を避けるため括弧は付けない。
  `++` は spec 09 では String 連結として登録されており、List 連結は
  現段階では prelude に含まれない）
- **Data type / type alias** — `T`, `Age` のような型名。型位置で
  hover すると `(data type)` / `(type alias)` のタグが付く。
  スキームは型側のため表示しない。
- **Local binder** — lambda / let / 関数パラメータ / case-arm パ
  ターン / do-bind の束縛。I6 は現状 top-level だけ scheme を
  back-annotate するので、**名前 + `(local)` タグ + 「型情報未
  取得」の注記** が出る。型表示は I6 が per-span `Ty` side table
  を拡張したあとに L4 側で対応（I-OQ96）。

## 動かないケース（L4 の制約）

- **別ファイルの定義の型は引けない**。LSP が開いている同じ
  `hello.sp` 内のみ `typeck::check_module` を回している。
  workspace scan は L6 以降（I-OQ72）。
- **Prelude の定義位置には飛べない** が、Prelude の型は
  `install_prelude` で揃えて登録されているので hover
  自体は動く。ソース位置への goto は I-OQ73 / I-OQ44 で検討中。
- **resolver エラーが 1 件でもあるファイル** では hover が抑止
  される（L5 goto と同じ理由：reference side table が得られない）。
  I-OQ74 で resolver API を改修すれば部分 hover が可能になる。
- **Type position hover のうち type variable（`a`, `b` …）** は
  `Resolution::Local` で scheme が紐付かないため `(local)` タグ
  のみ出る。binder をどこに紐付けるかの定義が固まるまで punt
  （I-OQ75）。
- **Typecheck が本体で失敗した関数** の top-level scheme は
  `inferred` に入らないので hover で型が出ないが、名前 + タグは
  表示される。typecheck エラー中の partial hover は今後の課題。

## ログで経路を確認

`SAPPHIRE_LSP_LOG=trace` を付けて VSCode の Extension Host を
起動すると、sapphire-lsp は stderr に以下の trace を吐く：

```
TRACE sapphire_lsp::server: textDocument/hover
  uri=file:///…/hello.sp line=48 character=10
```

キャレットを動かすたびにホバー要求が飛び、同じ行の trace が増える。
Response の中身は tower-lsp が自動で flush する。
