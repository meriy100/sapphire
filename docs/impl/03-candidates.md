# 03. ホスト言語候補

Sapphire コンパイラ本体を書く候補言語のプロファイル。`02-criteria.md`
の軸に沿って「強み」「弱み」「事例・参考」を記す。スコア付けは
`04-matrix.md` で。

候補の選定方針：**実績があり・ADT/パターンマッチを native か強
エミュレーションで持ち・Ruby との統合経路が現実的** な言語に絞
った。学術的 curiosity（Agda、Idris、Racket など）は除外。

---

## 候補 1: Rust

### 強み

- **ADT（`enum`）とパターンマッチ** が native。exhaustiveness
  検査が型システムに組み込まれている（C1）。
- **コンパイラ実装のエコシステムが厚い**：Rust 自身が Rust で書
  かれて以来、パーサライブラリ（`nom`、`pest`、`lalrpop`、`chumsky`）・
  AST walker・型検査器の参考実装が大量にある（C2）。
- **配布**：単一バイナリ生成、クロスコンパイルも比較的容易。
  `cargo install sapphire` で済む（C7）。
- **型システム** が強力。所有権モデルで AST のミスが compile
  時に見つかる（C5）。
- **パフォーマンス**：ネイティブコンパイル、起動も速い（C6）。

### 弱み

- **Ruby との相互運用** は外部プロセス経由が現実的。コンパイラ
  を Ruby から呼ぶ場合は `gem` として配布するが、内部は Rust
  バイナリ（例：`wasmtime` gem の構造）。学習コストあり（C3）。
- **Ruby プログラマの学習曲線は急**。所有権・ライフタイム・
  トレイトは Ruby 出身者には馴染みが薄い（C4）。
- **Self-hosting への乗り換えコスト大**：Rust で書いた parser
  を Sapphire に移植するのは遠い（C8、ただし低重み）。

### 事例・参考

- `gleam`（Erlang VM ターゲットの FP 言語）— Rust 実装。似た位
  置付けで参考になる。
- `roc-lang`（Elm 後継風言語）— Rust 実装。
- Ruby との統合例：`wasmtime-rb` gem、`rutie` / `magnus` /
  `rb-sys` crates（Rust バイナリを gem 経由で Ruby から呼ぶ型）。

---

## 候補 2: OCaml

### 強み

- **古典的なコンパイラ実装言語**。ADT・パターンマッチが native、
  Hindley-Milner が自然（C1, C5）。
- **実績**：Rust 自身の初期実装、Hack、ReScript、Flow、Coq、
  Haxe など多数のコンパイラが OCaml で書かれている（C2）。
- **コンパイル速度が極めて速い**（C6）。
- **バイナリ配布可能**、OPAM でインストール（C7、ただし Ruby
  コミュニティから見ると迂回路）。

### 弱み

- **日本語を含む学習リソース・コミュニティは Rust に比べると
  狭い**（C4）。
- **Ruby プログラマに対する親近感** は低い（関数型の書き方に慣
  れる必要がある）（C4）。
- **Ruby との相互運用** 経路は Rust と同程度の外部バイナリ扱
  い（C3）。
- エコシステムが小さく、パーサライブラリの選択肢は限定的
  （`menhir` が標準）。

### 事例・参考

- ReScript（Elm 系言語、Haxe 系ターゲット多数）が OCaml 実装。
- Flow（JavaScript 型検査器）が OCaml 実装。
- Rust の初期コンパイラが OCaml で書かれていた（self-host 移行
  を経た歴史）。

---

## 候補 3: Haskell

### 強み

- **Sapphire と血縁が近い**。07 MTC の設計は Haskell に寄せてお
  り、HKT・型クラス・do 記法・純粋性といった概念を **そのまま** 
  実装に落とし込める（C1, C5）。
- **コンパイラ実装のライブラリ** が豊富：`megaparsec`、
  `alex`/`happy`、`bound`、`generics-sop` 等（C2）。
- **型システムがもっとも強力**。GADT・型族等、コンパイラ内部の
  表現に役立つ道具が多い（C5）。
- **self-host への将来** ：Sapphire が Haskell 並の表現力を目指
  している以上、実装を Sapphire で書き直しても類似のコード構造
  になるはず（C8）。

### 弱み

- **ユーザ層との接続が弱い**：Ruby 出身者が Haskell を読み書きす
  るのは学習コストが大きい（C4）。Ruby 主体のコミュニティで
  maintainership を広く募る前提なら、Haskell を課すのは参入障壁
  になる。
- **配布** は Stack / Cabal と GHC toolchain が必要。バイナリは
  作れるが大きい（C7）。
- **コンパイル速度** は Rust・OCaml より遅い（C6）。
- **Ruby との相互運用** は外部プロセス扱い、または C API 経由
  （C3）。

### 事例・参考

- Elm の初期コンパイラが Haskell 実装（現在も）。
- PureScript が Haskell 実装。
- Idris2（Idris 自身で書き直された）の第 1 版は Haskell。

---

## 候補 4: Ruby

### 強み

- **target と同じ言語**。コンパイラが Ruby なら、生成した Ruby
  コードの検証や実行が **同プロセスで完結** する（C3、最大の利点）。
- **Ruby プログラマが即座に読める**。user / 将来の協力者が貢献
  しやすい（C4、最大の利点）。
- **配布**：`gem install sapphire` が自然、Bundler 統合も容易
  （C7、最大の利点）。
- **`:=` スニペット検査**：Ruby コード片を実際にパースしたり
  Ripper / `RubyVM::AbstractSyntaxTree` で検証したりできる。
  他言語では `ripper` 相当を外部呼び出しで使う必要がある（C3）。

### 弱み

- **ADT / パターンマッチが native でない**。Ruby 3.0+ の `case
  ... in` パターン構文は使えるが、**exhaustiveness 検査がない**
  のでコンパイラバグを呼びやすい（C1、最大の弱点）。
- **型システムが動的**。RBS / Sorbet で補えるが、コンパイラ実装
  の典型的なバグ（type visitor の網羅漏れなど）を言語レベルで
  防げない（C5）。
- **コンパイラ実装ライブラリが薄い**：Parslet・Racc が存在する
  が、Rust / OCaml / Haskell と比べると「コンパイラ書くための道
  具」の層が薄い（C2）。
- **パフォーマンス**：起動時間・コンパイル速度とも不利。大きな
  プロジェクトで辛くなる可能性（C6）。

### 事例・参考

- Ruby 製コンパイラ／パーサの実例：`prism`（CRuby parser、現在
  は C だが Ruby から多用される）、`parser` gem、`opal`（Ruby
  → JS コンパイラ）、`natalie`（Ruby → C++）、`rubyfmt`（formatter）。
- Opal は最も近い位置付け：Ruby を別の言語にコンパイルするもの
  を Ruby 自身で書いている前例。

---

## 候補 5: TypeScript

### 強み

- **discriminated union + `switch` + `never` によるパターンマッチ
  相当** が使える。TypeScript の strict mode ではかなり堅い（C1、
  native ではないが強エミュレーション）。
- **型システム** は強い方で、コンパイラ実装に耐える（C5）。
- **エコシステム**：`chevrotain`・`peggy`・`nearley` 等のパーサ
  ライブラリ。AST 処理の例も多い（TypeScript 自身が TS 実装）（C2）。
- **配布**：`npm install -g sapphire` で Node.js 経由。Ruby 層
  からは少し遠いが、広く使われている（C7）。

### 弱み

- **Node.js 依存**：Ruby コミュニティが対象なら Node.js ランタイ
  ムが必要になるのは余計な複雑さ（C3, C7）。
- **Ruby 相互運用** は外部プロセス扱い（C3）。
- **AST の表現が若干だるい**：discriminated union は使えるが、
  Rust / OCaml ほど簡潔にならない（C1）。

### 事例・参考

- TypeScript コンパイラ自身が TS 実装。
- Deno / Bun 等の JS ランタイムも TS で書かれている部分あり。
- Elm 以前の JS 系関数型言語（Fable、ReScript）は OCaml だが、
  最近のツーリングで TS は多い。

---

## 候補 6: Crystal

### 強み

- **Ruby 風の構文**。Ruby プログラマが読み書きしやすい（C4、Ruby
  と並ぶ強み）。
- **ADT（`enum` + case）とパターンマッチ** を持つ。Ruby より堅
  い型システム（C1、C5）。
- **静的にコンパイルしてバイナリ**。Ruby の書き味で Rust に近い
  パフォーマンス（C6, C7）。

### 弱み

- **エコシステムが小さい**：パーサライブラリ・コンパイラ参考実
  装が Rust / OCaml / Haskell に比べて薄い（C2）。
- **成熟度**：Crystal 自身のコンパイラは self-host 済で実績あり
  だが、それ以外に大規模コンパイラ実装の事例が薄い。周辺ライブ
  ラリ・知見の蓄積も限定的（C2）。
- **Ruby との相互運用** は外部プロセス扱い（C3、Ruby 風でも
  Ruby そのものではない）。
- **日本語コミュニティは限定的**（C4、Ruby に比べると）。

### 事例・参考

- Crystal 言語のコンパイラ自身が Crystal で書かれている（self-
  host 達成済み）。
- Lucky フレームワーク・Kemal など Web 系。
- Sapphire 的には「Ruby に感覚が近い静的型言語で書く」選択肢
  として唯一の候補。

---

## 除外した候補（参考）

| 言語 | 除外理由 |
|---|---|
| Python | Ruby と似た弱みを持つが Ruby ほど target に近くない。わざわざ選ぶ理由が乏しい |
| Scala / F# | JVM / .NET 依存が重い。Ruby コミュニティから遠い |
| Elixir | BEAM VM 依存。Ruby 風ではあるが同時代のコンパイラ実例少 |
| Java | 手続き的で ADT が辛い。除外 |
| Go | generics は入ったが sum type がなく、コンパイラ実装では Rust に対して劣る |
| 学術系（Agda・Idris 等） | 学習コストと成熟度で除外 |

候補を追加したい言語があればここに加えて profile を書き足す。
