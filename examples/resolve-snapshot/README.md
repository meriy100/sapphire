# resolve-snapshot

I5 リゾルバ（`crates/sapphire-compiler/src/resolver/`）が M9 例題プ
ログラム（`docs/spec/12-example-programs.md` / `examples/sources/`）
を名前解決した結果のスナップショット集。

仕様：

- `crates/sapphire-compiler/examples/resolve_dump.rs` が生成する
  plain-text 出力を保存している。フォーマットは
  `ResolvedProgram` の module 順に、各モジュールの
  - トップレベル宣言一覧（`pub/priv` 可視性 + 名前空間 + kind）
  - エクスポート（外から見える名前と resolved reference）
  - unqualified scope（import と prelude で持ち込んだ名前）
  - qualifier（モジュール名・`as` alias）
  - references（ソース上の参照箇所 → 解決結果、`loc start..end`）
- 入力は I3 のレキサ → I4 のレイアウト解決パス → I4 のパーサ → I5
  のリゾルバを一気通貫に通したもの。

## ファイル対応

| 入力                                                    | スナップショット                                      |
|---------------------------------------------------------|------------------------------------------------------|
| `examples/sources/01-hello-ruby/Main.sp`                | `01-hello-ruby-Main.resolved.txt`                    |
| `examples/sources/02-parse-numbers/NumberSum.sp`        | `02-parse-numbers-NumberSum.resolved.txt`            |
| `examples/sources/03-students-records/Students.sp`      | `03-students-records-Students.resolved.txt`          |
| `examples/sources/04-fetch-summarise/`（2 モジュール）  | `04-fetch-summarise.resolved.txt`                    |

Example 4 は `Http.sp` と `Fetch.sp` を **同時** に resolver へ渡し、
単一の `ResolvedProgram` として解決した結果を 1 ファイルに収めて
いる。`Fetch` 内の `HttpError` や `NetworkError` 参照が `Http`
モジュールの対応する宣言に解決されているのが references セクショ
ンで確認できる。

## 再生成

リポジトリルートから：

```
for src in \
  01-hello-ruby/Main.sp \
  02-parse-numbers/NumberSum.sp \
  03-students-records/Students.sp
do
  dir=$(dirname "$src")
  base=$(basename "$src" .sp)
  cargo run -q -p sapphire-compiler --example resolve_dump -- \
    "examples/sources/$src" \
    > "examples/resolve-snapshot/${dir}-${base}.resolved.txt"
done

cargo run -q -p sapphire-compiler --example resolve_dump -- \
  examples/sources/04-fetch-summarise/ \
  > examples/resolve-snapshot/04-fetch-summarise.resolved.txt
```

差分を確認したい場合：

```
cargo run -q -p sapphire-compiler --example resolve_dump -- \
    examples/sources/01-hello-ruby/Main.sp \
  | diff -u examples/resolve-snapshot/01-hello-ruby-Main.resolved.txt -
```

## 出力の読み方

各モジュールのヘッダ `== module X ==` のあと、5 つのセクションが
順に並ぶ。

- **top-level**：`pub/priv 可視性 / 名前空間 / 名前 / [kind]`。
  `value` はふつうの値束縛 / `ctor` はデータコンストラクタ /
  `method` はクラスメソッド / `ruby` は `:=` 形式の Ruby 埋め込み /
  `data` / `alias` / `class` は型側の宣言。
- **exports**：モジュール外から見える名前だけ。`bare Maybe`
  エクスポートがコンストラクタを隠す、`Maybe(..)` が全部公開する、
  といった spec 08 §Visibility の規則はここに反映される。
- **unqualified-scope**：import で unqualified に取り込んだ名前。
  08 の暗黙 prelude import（09 §The prelude as a module）がほぼ
  全てのモジュールで大量の prelude 名を載せる。
- **qualifiers**：`Http`、`Http as H`、`Prelude` のように qualified
  アクセスが効く接頭辞。モジュール自身は常に自分のフル名で見える。
- **references**：ソース位置 `start..end` → 解決結果。`local x` は
  lambda / let / パターンなどで束縛されたローカル、`global Mod.x`
  は別モジュール（または自モジュールのトップレベル）の定義。

## 含まれないもの

- **型検査**。`Scheme` / `Type` は構文上の形のまま保持され、
  I5 は自由な型変数（`a`、`b` など）を Local として記録するに
  とどまる。kind 整合や主要型の推論は未実施。
- **糖衣展開**。`[x, y, z]` は `Expr::ListLit` のまま、`if` は
  `Expr::If` のまま。`case` への脱糖は elaboration 層で行う。
- **インスタンス検証**。`instance C T where ...` の class / head
  の名前は resolve するが、class membership（method が class に
  宣言されているか）のチェックは I6c の領分。

これらは I6（type checker）以降の仕事。
