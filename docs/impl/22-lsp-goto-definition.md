# 22. LSP goto-definition（L5）

Track L の L5 タスクとして、`sapphire-lsp` サーバに
`textDocument/definition` 経路を追加する。L1（scaffold）〜L3
（incremental sync）で築いたドキュメントストアと LineMap を土台に、
I5 resolver が残した reference side table を引いて「識別子の上で
F12 を叩くと同じファイル内の定義位置に飛ぶ」体験を最小機能で実装
する。本文書は実装と同じ commit に載せる設計メモで、後続の
L4（hover）/ L6（completion）で同じ reference lookup を再利用する
前提も明記しておく。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| capability | `definition_provider = Some(OneOf::Left(true))`（`LocationLink` ではなく `Location` を返す） |
| 返却 shape | `GotoDefinitionResponse::Scalar(Location)` 単一 |
| 解析 | `analyze(text)` で AST を得て `resolve(module)` を再走させる（naive、L3 と同様 full reparse） |
| Position → byte | L2 で用意した `LineMap::byte_offset` をそのまま使う |
| Span → 定義位置 | `ResolvedModule.references: HashMap<Span, Resolution>` を走査し、`byte_offset` を最も狭く包含する span を選ぶ |
| Local binding | resolver の side table が span を持たないので、AST を追加で walk して innermost 束縛を探す |
| Global binding（同 module） | `ResolvedModule.env.top_lookup(name, ns)` で `TopLevelDef.span` を引き、宣言ヘッダから **識別子 span** を narrow する |
| Cross-module / Prelude | **諦めて `None` を返す**（L5 スコープ外、I-OQ72 / I-OQ73） |
| resolve 失敗時 | **諦めて `None` を返す**（side table が得られないため） |
| race guard | L3 `analyze_and_publish` のような version 比較は入れない — goto は request-response で client が stale 応答を吸収できる |

## アルゴリズム

```
resolve_position_to_location(uri, pos)
  1. text = documents.get(uri)?                          # DashMap
  2. analysis = analyze(text)                           # lex/layout/parse
  3. module = analysis.module?                           # None if parse failed
  4. resolved = resolve(module.clone()).ok()?            # None if resolve failed
  5. line_map = build_line_map(text)
  6. byte_offset = line_map.byte_offset(pos)?            # UTF-16 → byte
  7. find_definition(module, resolved, text, byte_offset, line_map)?
  8. → Location { uri, range }
```

`find_definition` は純粋関数：

```
find_definition(module, resolved, source, byte, map)
  span = find_reference_span(resolved.references, byte)?
  res  = resolved.references[span]
  def  = resolve_definition_span(module, resolved.env, source, span, res)?
  Some(map.range(def))
```

### Position → 参照 span の選び方

`resolved.references: HashMap<Span, Resolution>` は reference site の
span をキーにしている。`byte_offset` を **最も狭く包含する** span
を選ぶ：

- 半開区間 `[start, end)` を採用（L2 range と揃える）。空 span
  `(start == end)` は `byte == start` のみ hit。
- 複数が hit した場合、`span.end - span.start` が最小のものを採る。

複雑度は `O(n)`（`n = references.len()`）。M9 例題規模（数百行、
reference 数は数十〜数百）なら線形 scan で十分。`byte_offset` を
昇順 index にする最適化は将来（I-OQ72 と合わせて評価）。

### Global → TopLevelDef.span を narrow する理由

resolver は `TopLevelDef.span` に parser が作った **ヘッダ全体の
span** を格納する（例：`plus : Int -> Int` ならシグネチャ末尾まで）。
そのまま LSP `Range` に流すと「定義行の長い範囲」がハイライトされ
る。ユーザ体験としては **識別子そのもの** がハイライトされる方が
素直なので、decl kind 別に以下の narrow を行う：

| decl kind | span.start の byte | narrow 戦略 |
|---|---|---|
| `Signature` | 識別子の頭 | `start..start+name.len()` |
| `Value` | 識別子の頭 | `start..start+name.len()` |
| `Data`（型名） | `data` キーワード頭 | `+ "data ".len()` から name 長 |
| `Data` ctor | ctor 名の頭 | `c.span.start..+name.len()` |
| `TypeAlias` | `type` キーワード頭 | `+ "type ".len()` から name 長 |
| `Class` | `class` キーワード頭 | 現状 `header_span` 全体（narrow 未対応） |
| `ClassMethod` | method 名の頭 | `mspan.start..+name.len()` |
| `RubyEmbed` | 識別子の頭 | `start..start+name.len()` |

narrow 失敗時は `header_span` をそのまま返す（LSP クライアントは
広めの range でも goto 自体は動く）。**source からの再 lex で
識別子 span を求める案** もあるが、decl kind ごとに固定 prefix が
あるケースがほとんどで十分精度が出るのでまずはこちらで。

### Local binding の walk

I5 resolver は `Resolution::Local { name }` に **束縛側 span を
持たない**（reference side table の value には name しか入らない）。
span を引くには、束縛可能な node を enclosing scope 順に巡る
walk が必要。`LocalFinder` は resolver の `Walker` と同じ経路を
辿りつつ、「`name` 一致 かつ scope が `ref_span.start` を包含」する
binder の中で **`binding_span.start` が最大** のものを選ぶ。

この "start 最大" が innermost 選択に相当する：Sapphire の束縛は
lexical scope で、内側の束縛は必ず外側より source 後方に現れる。

対応する binder：

- `ValueClause.params` の `Pattern::Var { span }`
- `Expr::Lambda.params` の `Pattern::Var { span }`
- `Expr::Let { name, span }` → span から `let` キーワードを skip
  して名前 byte を探す
- `Expr::Case` の arm pattern の `Pattern::Var { span }`
- `Expr::Do` 内の `DoStmt::Bind` の `Pattern::Var { span }`
- `Expr::Do` 内の `DoStmt::Let` の名前 — source を scan して literal
  byte を探す
- `RubyEmbed` の `Param { span }`

`let`-keyword skip / literal-scan は `locate_name_after_keyword` /
`locate_name_literal` ヘルパで実装している。ASCII identifier
境界（`alnum` / `_` / `'`）をチェックすることで `let` キーワード直後
の partial マッチを避ける。

**type variable は L5 対象外**。resolver は `Type::Var` にも
`Resolution::Local` を残すが、`forall` と暗黙的 quantifier の両方が
あり、どこで「束縛された」とみなすかが実装選択に依存する。L5 で
は type position の local reference が来たら `find_local_binding`
が binder を見つけられず `None` を返す（fall-through で goto 抑止）。

### Cross-module / Prelude を扱わない理由

- **Cross-module**：LSP が 1 file/1 document 前提で動いている
  （DashMap<Url, Document>）。他 module の source を開いていない
  場合、定義 span があっても `Url` が作れない。workspace ルート
  から `.sp` を scan するインフラは L6 以降（I-OQ72）。
- **Prelude**：`resolver/prelude.rs` の静的テーブル。`lib/Prelude.sp`
  が存在しないので `Url` が作れない。将来 `.sp` 化（I-OQ44）に
  併せて再検討（I-OQ73）。

## resolve 失敗時の諦め

現行の `resolve` は `Result<ResolvedProgram, Vec<ResolveError>>` を
返す。1 件でもエラーがあると `Err` に落ち、部分的 references は
取り出せない。L5 はこの形に合わせて **resolve 失敗時に `None`** を
返す方針。

別案として「resolver が `ResolveError` と `ResolvedModule` の
両方を返せる API を足す」があるが、resolver 本体への侵襲で本
タスクのスコープを越える。資源が取れたら I-OQ74 で再評価。

## race guard を入れない判断

L3 diagnostics は publish 経路のため「古い解析結果で新しい
diagnostic を上書きする」race があり、`analyze_and_publish` の
version guard が必要だった。goto は request-response で client が
常に最新の応答を取るため、server 側に guard は要らない。また
`documents.get(uri)` でスナップショットを取ってしまえばその後の
edit と analyze は完全に独立する。

## テスト戦略

- **Unit（`src/definition.rs::tests`、17 本）**
  - 同一 module 内 top-level value / signature / type / ctor
  - let / lambda / 関数 param / case-arm / do-bind / nested let
  - prelude 参照は `None`
  - source 末尾超過は `None`
  - overlap 時に innermost を選ぶ
- **Integration（`tests/example_goto.rs`、6 本）**
  - `examples/lsp-goto/hello.sp` を analyze + resolve してから
    `find_definition` を回す。関数間 / 型 / ctor / let / prelude
    を代表的にカバー。

`cargo test --workspace` で合計 88 本以上が通ることを acceptance
とする。

## L4 / L6 への引き継ぎ

L5 の成果物が後続で効くポイント：

- **reference lookup の再利用**：`find_reference_span` は hover
  / signature-help / completion で **カーソル直下の識別子** を
  決めるときにそのまま使える。
- **TopLevelDef.span の narrow**：L4 hover が「宣言 1 行を
  tooltip に出す」なら、narrow を外して header span を使える。
  同じ decl kind 分岐が再利用できる。
- **Cross-file goto を入れるとき**：`Location` を `uri` ごと
  返す形は既に整っているので、ドキュメントストアを workspace
  scan と同期させる層を L6 以降で足せば goto の side は差分が
  小さい。

## 新規 OQ

本タスクで浮上した未決事項：

- **I-OQ72 Cross-file goto / workspace scan**：`import Foo`
  先の定義へ飛ぶには、workspace ルートから `.sp` を発見・
  キャッシュする層が必要。L5 は同一ファイル限定。
- **I-OQ73 Prelude 定義への goto**：現状 Prelude は静的
  テーブル。`.sp` 化（I-OQ44）が済むまで goto できない。
- **I-OQ74 resolve 部分成功の exposing**：resolver を
  `Result<_, Vec<_>>` から `(ResolvedProgram, Vec<Error>)`
  形に変え、エラーがあっても reference side table を使えるよう
  にする改修。L5 は resolve 成功時のみ goto が走る。
- **I-OQ75 type-position goto**：`Type::Var` / explicit
  `forall` / 暗黙 quantifier の三択があり、「どこが binder か」
  の定義が多義的。type inference（I6）で詳細を決めてから
  再評価する。

I-OQ43（span key 衝突）は本タスクで顕在化しなかったが、
`find_reference_span` が「最狭 span 選択」で暫定回避している旨を
明記しておく（衝突した場合は任意 pick になるが、M9 例題では
観測されない）。

## 参照

- `07-lsp-stack.md` — LSP stack 選定
- `10-lsp-scaffold.md` — L1 scaffold
- `17-lsp-diagnostics.md` — L2 の LineMap / UTF-16 変換
- `21-lsp-incremental-sync.md` — L3 の差分適用と document store
- `15-resolver.md` — I5 resolver と reference side table の設計
- `../open-questions.md` §1.5 — I-OQ9 / I-OQ43 / I-OQ72〜I-OQ75
