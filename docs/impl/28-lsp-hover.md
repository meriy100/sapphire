# 28. LSP hover（L4）

Track L の L4 タスクとして、`sapphire-lsp` サーバに
`textDocument/hover` 経路を追加する。L1〜L3（scaffold / diagnostics
/ incremental sync）で築いた document store と `LineMap`、L5
（goto-definition）で整えた reference lookup を再利用し、I6 HM 型
検査の結果を引いて **カーソル直下の識別子の型スキームを popup
表示** する体験を最小機能で実装する。本文書は実装と同じ commit に
載せる設計メモで、後続の L6（completion）や signature help に同じ
pipeline を流用する前提も明記しておく。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| capability | `hover_provider = Some(HoverProviderCapability::Simple(true))`（registration options は使わない） |
| 返却 shape | `Hover { contents: HoverContents::Markup(MarkupContent { kind: Markdown, value }), range: Some(ref_span_range) }` |
| 解析 | `analyze(text)` → `resolve(module)` → `typeck::infer::check_module` を都度 full-reparse（L3 / L5 と同じ naive 方針） |
| Position → byte | L2 の `LineMap::byte_offset` をそのまま使用 |
| Span → 型 | 参照 span の resolution を `HoverTypes { inferred, ctors, globals }` で引く。`inferred` は `InferCtx.inferred: HashMap<String, Scheme>`、`ctors` は `TypeEnv.ctors: HashMap<String, CtorInfo>`、`globals` は `TypeEnv.globals: HashMap<GlobalId, Scheme>`（prelude operator / 関数、user class method など inferred に載らないエントリをカバー） |
| Local binder | I6 は per-span の `Ty` side table を出していないため、当座は **名前 + `(local)` タグ + 「型情報未取得」注記** で返す（I-OQ96） |
| typecheck 失敗時 | `check_module` がエラーでも `ctx.inferred` を projection して返す（partial hover）。hover は「editor セッションで best-effort に出したい」用途のため、clean compile を gate にしない |
| Range | reference span の narrow な範囲（識別子そのもの）を `LineMap::range` で変換して返す |
| race guard | request-response のため不要（client が stale 応答を吸収する） |

## Pipeline（L5 との共有）

```
resolve_position_to_hover(uri, pos)
  1. text     = documents.get(uri)?
  2. analysis = analyze(text)
  3. module   = analysis.module?
  4. resolved = resolve(module.clone()).ok()?
  5. typed    = collect_hover_types(module_name, &module)
      ├── InferCtx::new(module_name)
      ├── install_prelude(&mut ctx)
      ├── check_module(&mut ctx, &module)  // エラーでも続行
      └── HoverTypes { inferred, ctors, globals }
  6. line_map = build_line_map(text)
  7. byte     = line_map.byte_offset(pos)?
  8. find_hover_info(module, resolved, typed, text, byte, line_map)?
```

`find_hover_info` は純粋関数：

```
find_hover_info(module, resolved, typed, source, byte, map)
  span = find_reference_span(&resolved.references, byte)?  // L5 と共有
  res  = resolved.references[span]
  info = build_hover_info(&resolved.env, &typed, &res)?
  md   = render_markdown(&info)
  Some(Hover { contents: Markup(md), range: map.range(span) })
```

### Reference lookup を L5 と共有する理由

L5 で導入した `find_reference_span(&HashMap<Span, Resolution>, byte)`
は「最狭 span 選択」で overlap 時に innermost を拾う。hover も同じ
要求（`BinOp` の演算子位置を `App` より優先、など）を持つので、
`definition.rs` の private helper を `pub(crate)` に昇格して再利用
している。`Resolution` の分岐も共通で、goto との差は **定義位置の
span** を返すか **型スキームを取るか** だけになる。

### `HoverTypes` の shape

I6 の `check_module` / `install_prelude` / `register_ast_class` は
schemes を **複数のテーブル** に分散して書き込む：

| 書き込み先 | 書き込むもの | 書き込む箇所 |
|---|---|---|
| `InferCtx.inferred: HashMap<String, Scheme>` | 現在モジュールの top-level value / signature / Ruby-embed のスキーム | `check_module` Phase D / Ruby-embed ループ |
| `TypeEnv.ctors: HashMap<String, CtorInfo>` | data constructor（user / prelude 両方） | `register_ast_data` / `install_prelude` |
| `TypeEnv.globals: HashMap<GlobalId, Scheme>` | 上記すべて **に加えて** prelude operator (`+`, `++`, `>>=`, …) / prelude 関数 (`map`, `pure`, …) / user class method | `install_prelude` の `add_prelude` / `register_ast_class` |

特に **prelude operator / prelude 関数 / user class method は
`inferred` に載らず、`globals` にのみ存在する**。bare name で
`inferred` を引いても hit しないので、L4 では `GlobalId { module,
name }` キーで `globals` を引く経路を用意する必要がある。

この分散を踏まえて L4 は 3 projection を projection した
`HoverTypes { inferred, ctors, globals }` を受け取る：

- `DefKind::Value` / `DefKind::RubyEmbed` / `DefKind::ClassMethod`
  → `globals[GlobalId(current_module, name)]` → fallback `inferred[name]`
- `DefKind::Ctor`（user / prelude） → `ctors[name]`
- `DefKind::DataType` / `DefKind::TypeAlias` / `DefKind::Class`
  → **scheme なし**、タグのみ表示（fallback の「_型情報未取得_」注記
  は type-side context では抑制、下記 §Hover content の整形）
- `DefKind::*` で top 見つからない imported / prelude 名 → `ctors` →
  `globals[GlobalId("Prelude", name)]`（ないし imported モジュール） →
  `inferred[name]` の順で bare name / qualified 両方を試行

`HoverTypes` を新しい struct にしたのは：

1. I6 が将来 per-span `Ty` side table（I-OQ57）を追加したときに
   `HoverTypes` にフィールドを増やすだけで `find_hover_info` の
   signature が変わらないようにするため。
2. typeck 本体を改変せずに済ませるため。既存の
   `InferCtx.inferred` / `type_env.ctors` / `type_env.globals` を
   owning clone で projection するだけで、typeck 側のコード変更なし。

**projection か `InferCtx` 全体参照か** — 現状は projection 3 テー
ブルで足りているが、I6 が新たな side table（per-span `Ty`、class
instance dict metadata 等）を増やすたびに `HoverTypes` を広げる圧が
かかる。projection を続けるか `Arc<InferCtx>` 相当を持ち回るかの判
断は I-OQ100 で追跡。

### Prelude と同一モジュール名 lookup の整合

`install_prelude` は `ctx.module = "Prelude"` で prelude data /
class / value を登録してから、`check_module` で
`ctx.module = module_name`（例：`Main`）に切り替える。L4 は
resolver の `Resolution::Global(r)` で `r.module.segments ==
["Prelude"]` を真偽値で判定し、**スキーム自体は
`globals[GlobalId("Prelude", r.name)]` で qualified に引く**（prelude
operator / 関数は `inferred` に載らないため）。ctor は別テーブル
`ctors` に bare name でも残っているので、`ctors` → `globals` →
`inferred` の順で lookup し、最初に hit したものを返す。

将来 Prelude を `.sp` 化（I-OQ44）したときは、`check_program` で
複数モジュールの scheme を並行して保持する形に移行する想定（L6
で cross-file 拡張するタイミングと揃えられる）。

## Hover content の整形

Markdown content type を採用し、次の 2 パートで組み立てる：

```
```sapphire
<name> : <scheme.pretty()>
```
_(<context-tag>)_
```

- code fence の言語タグは `sapphire`。VSCode 拡張が TextMate
  grammar を提供しているので、pretty-printed scheme の演算子
  （`->`, `=>`）や keyword（`forall`）にもそこそこハイライトが効く。
- context-tag は italic の 1 行で添える。spec 08 の "home module"
  風の表記ではなく、ユーザー視点の分類を優先：
  - `(top-level value)` — 同一モジュールの value binding
  - `(constructor of `T`)` — user / prelude ctor
  - `(method of class `C`)` — class method
  - `(`:=`-binding)` — `:=` binding（CommonMark renderer に優しい
    形、バッククォート内に `:=` のみを入れる）
  - `(data type)` / `(type alias)` / `(class)` — 型位置
  - `(local)` — lambda / let / pattern binder
  - `(prelude)` — Prelude の定義
  - `(imported from `M`)` — 他モジュールの import（scheme 無し）
- **演算子名は symbol のまま**（`+ : Int -> Int -> Int`）書く。
  Haskell の section 記法 `(+)` と混同させないため括弧は付けない。
- scheme が引けなかった場合、**value-side の context に限り**
  `_型情報未取得_` を下に添える（`Local` / `External` など）。
  `DataType` / `TypeAlias` / `Class` は設計上 value-level scheme を
  持たないため、これらの context では fallback 注記を抑制し、タグ
  だけで完結させる（"scheme が取れなかった" は事実と乖離する）。
  hover を silent に消さないのは「あなたは識別子を正しく認識して
  います、型情報は今は無いだけです」を見せてデバッグしやすくする
  ため。

## Local binder の型表示を punt する

I6 の HM inferencer は **top-level binding の scheme** と
**constraint residue** を出すが、lambda / let / pattern / do-bind
の local を spanned AST に back-annotate しない。`docs/impl/18-
typecheck-hm.md` §I7 codegen との接続インターフェース（= I-OQ57）は
"I7 着手時に判断" としており、L4 時点では API が無い。

したがって L4 は **local references を `(local)` タグ + 名前のみ** で
返し、I6 が per-span `Ty` を露出した後に L4 側を拡張する。新規 OQ
**I-OQ96** で追跡（下記 §新規 OQ）。既存 test はそのまま green を
維持するので、拡張は L4 への追加だけで済む。

## typecheck エラー時の挙動

`check_module` が `Err(errors)` を返したとしても、`ctx.inferred` は
**エラーが発生する前に終えた binding の scheme** を保持したままに
なる。L4 はこの partial 結果を projection して返すため、1 件の
typeck エラーで file 全体の hover が消えることはない：

- `foo : Int` / `foo = 1` が通っていれば `foo` 上で hover できる
- 同時に `bar = invalidExpr` が失敗していても上記に影響しない

この挙動は L5 goto が resolver 失敗時に諦める（I-OQ74）のと対照
的だが、理由は明確：resolver が失敗すると reference side table
が得られない（lookup する鍵が無い）のに対し、typecheck は
`ctx.inferred` がエラー時でも残り、lookup できる。

## 新規 OQ

本タスクで浮上した未決事項：

- **I-OQ96 Local binding 型の hover 表示**：I6 は top-level
  scheme しか back-annotate しない。L4 は当座 "名前 + `(local)` +
  『型情報未取得』" を返す。I-OQ57（typed AST の持ち方）で
  `HashMap<Span, Ty>` が入ったら L4 拡張する。
- **I-OQ97 Hover キャッシュ / incremental typecheck**：現状は
  keystroke ごとに `check_module` を full re-run する。L3 が text
  sync を incremental 化したのと同様に、hover / completion 用の
  type info キャッシュが欲しくなる。L6 / I-OQ9 と連動して
  再評価。
- **I-OQ98 Prelude binding の docstring**：現状 `install_prelude`
  は scheme のみで documentation 文字列を持たない。spec 09 の
  prelude 定義に doc comment を生やし、hover の Markdown に
  2 段目として差し込めるようにしたい。I-OQ44（Prelude の `.sp`
  化）と合わせて実装する想定。
- **I-OQ99 Type-position hover の挙動**：type variable（`a`,
  `b` …）や `forall` 量化子位置での hover はまだ name-only。
  L5 goto の I-OQ75 と paired で、I6 が `forall` / 暗黙
  quantifier の束縛位置を固めてから拡張する。
- **I-OQ100 `HoverTypes` の projection 戦略**：現状は `inferred`
  / `ctors` / `globals` の 3 projection を clone する。I6 が新規
  side table（per-span `Ty`、dict metadata 等）を追加するたびに
  `HoverTypes` を広げる圧がかかる。projection を続けるか、LSP
  セッション全体で `Arc<InferCtx>` を抱える形に切り替えるかの
  判断条件を I-OQ57（typed AST の持ち方）着地時に決める。

## 今後の拡張

- **Data ctor の field 別 hover**：record constructor や
  record field access で field type を引く。spec 04 §Records の
  構造的 record と絡むので I6 の field access resolution と
  paired で実装。
- **Docstring 連動**：spec 09 の prelude エントリや user 関数の
  先頭コメント（spec 02 §Line comments）を AST に attach して、
  hover Markdown の 2 段目に出す。
- **Inlay hints への発展**：hover の情報密度が上がったら、
  inlay hints で関数引数の型や let 束縛の型を inline 表示する
  capability（`textDocument/inlayHint`）に派生させる。L7 以降。

## 参照

- `07-lsp-stack.md` — LSP stack 選定
- `10-lsp-scaffold.md` — L1 scaffold
- `17-lsp-diagnostics.md` — L2 の `LineMap` / UTF-16 変換
- `21-lsp-incremental-sync.md` — L3 の差分適用と document store
- `22-lsp-goto-definition.md` — L5 の reference lookup（L4 と共有）
- `18-typecheck-hm.md` / `19-typecheck-adt.md` / `20-typecheck-classes.md`
  — I6 の HM inferencer と typeck 出力
- `../open-questions.md` §1.5 — I-OQ9 / I-OQ57 / I-OQ73 / I-OQ75 /
  I-OQ96〜I-OQ100
