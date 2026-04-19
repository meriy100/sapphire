# 17. LSP diagnostics（L2）

Track L の L2 タスクとして、`sapphire-lsp` サーバに **ドキュメント
ストア** と **`textDocument/publishDiagnostics`** を入れる。L1 の
scaffold（`docs/impl/10-lsp-scaffold.md`）に対して「フロントエンド
エラーをエディタへ届ける最小経路」を追加するもので、本文書は
実装と同じ commit に載せる設計メモ。

後続 L3（incremental sync）/ L4（hover）/ L5（goto-definition）/
L6（completion）は、本文書で置いた document store と解析ドライバを
そのまま拡張していく想定。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| 再解析モデル | **full reparse**（`analyze(text)` を毎回走らせる）。差分適用はしない。 |
| ドキュメントストア | `DashMap<Url, Document>`（`Document { text: String, version: i32 }`）。 |
| エラー収集 | **1 エラー / 1 ファイル**。parser に recovery を入れていないので、最初の lex / layout / parse 失敗で止まる。 |
| エラー envelope | `sapphire_compiler::error::CompileError { kind, span }` 新設。`LexError` / `LayoutError` / `ParseError` を同一型に束ねる。 |
| 解析エントリポイント | `sapphire_compiler::analyze::analyze(src) -> AnalysisResult { module, errors }` 新設。 |
| 座標系 | `Span` は **byte offset**（現行維持）。LSP 側へは **UTF-16 code unit** 基準の `Position` に変換する。 |
| 変換実装 | 自前 `LineMap`（`sapphire-lsp::diagnostics`）。`line_starts: Vec<usize>` を 1 回スキャンで作って binary search。 |
| Diagnostic.code | `sapphire/lex-error` / `sapphire/layout-error` / `sapphire/parse-error`（`NumberOrString::String`）。 |
| Diagnostic.source | `"sapphire"` 固定。 |
| Diagnostic.severity | `Error` 固定（L2 スコープでは warning 段階のエラーは出ない）。 |
| `didClose` の挙動 | `documents.remove` → `publish_diagnostics(uri, [], None)` で明示的にクリア。 |
| エラー無しの挙動 | 空 `[]` を publish して以前の diagnostic を消す。 |

## なぜ full reparse か（I-OQ9 再訪）

L0（`07-lsp-stack.md`）で「初期実装は naive 再解析から始め、Salsa
等のインクリメンタル基盤は後段」と punt 済。L2 ではその punt を
そのまま引き継ぐ：

- 現行パーサは hand-written recursive descent（I-OQ2 DECIDED）で、
  AST を部分的に保持するインタフェースを持っていない。インクリメン
  タル再解析を足すには AST の永続化と依存追跡が必要で、L2 の
  ゴール（**parse error を拾って返す**）に対して明らかに過剰。
- M9 例題の規模なら full reparse のレイテンシが人体感知できる閾値
  に乗らない。1 モジュール数百行の分量で `analyze` の所要は
  マイクロ秒〜ミリ秒オーダー。
- インクリメンタル化するとしても、候補は「テキスト差分を適用した
  あと rope に対して tree-sitter 類似の局所再解析」か「Salsa で
  関数レベル memoize」の 2 択になる。前者は AST 構造への侵襲が
  大きく、後者は pass 構造の再設計を伴う。L2 単独で決められる話で
  はないので、実測で性能問題が露呈してから I-OQ9 を決着させる。

本 L2 では `did_change` のたびに `analyze(text)` を呼び、
`AnalysisResult` の `errors` を LSP diagnostic に project する。
デバウンス（連続 change を潰す）も入れない — full reparse が十分
速い間は素朴な即時再解析で問題にならず、遅くなったら tokio task
を spawn + abort する標準パターンで足せる。

## LSP UTF-16 と byte offset の変換

LSP 3.17 `Position` は `character` を **UTF-16 code unit** で数える
のがデフォルト（`positionEncoding` を negotiate すれば UTF-8 にも
倒せるが、VSCode は依然として UTF-16 が基本）。一方 Sapphire の
`Span` は byte offset（spec 02 §Source text は UTF-8）。つまり
毎回:

- **byte offset → line** は `line_starts: Vec<usize>`（各行の開始
  byte offset）を作って binary search で O(log L)。
- **line_start..byte offset の UTF-16 幅** は `&source[line_start..
  byte]` を slice して `char::len_utf16` を足し合わせる。1 行の
  長さは現実的には数百バイトで、per-call O(行長) は許容。

自前実装にした理由：

- 依存を足さずに済む。`line-index` / `lsp-text` のような crate は
  ある（rust-analyzer が使っている）が、**L2 スコープでは line
  map を作って UTF-16 幅を数える以上の機能を使わない**。
- Sapphire の Span 型は `{ start: usize, end: usize }` だけで、
  `line_index::TextSize` 相当の newtype も持っていない。変換
  レイヤの薄さを優先した。
- 将来インクリメンタル化で line map の差分更新が必要になった段階で、
  crate 採用を再評価する（I-OQ53）。

同じ `LineMap` を 1 リクエスト内で複数の diagnostic 変換に使い
回すため、`compile_errors_to_diagnostics(errors, source)` は
source から `LineMap` を 1 回作って各 error に適用する形にした。

## diagnostic code の命名

`CompileError::code()` が `&'static str` を返す。値：

- `sapphire/lex-error`
- `sapphire/layout-error`
- `sapphire/parse-error`

名前空間 `sapphire/` を予約し、**将来 sub-category を足す余地を
残す**（例：`sapphire/lex-error/unterminated-string`）。現状は
top-level の 3 値のみで十分なので、スラッシュ 2 段目は予約のみ。

LSP の `Diagnostic.code` は `NumberOrString` なので、文字列で
置く方が pattern match の将来 code action 設計と噛み合う。数値
ID は採らない（OpenAPI スタイルのエラー番号管理はここではやらない）。

`source = "sapphire"` は固定。これで VSCode の **問題** パネルでの
表示が `[sapphire] sapphire/parse-error: ...` になり、他の LSP が
同じファイルに noop 拡張を入れている場合でも衝突しない。

## エラー recovery を今回入れない判断

パーサに panic mode / error productions を足すことは本 L2 では
やらない：

- recovery の設計は "構文エラーの後、どこまで飛ばして再開するか" の
  heuristic であり、spec を読み直す作業を伴う。L2 のスコープから
  外れる。
- VSCode の diagnostic は "エラーが 1 件でも見える" 側が優先。1 行
  1 ファイルあたり error が 1 件しか出ない状態でも、赤下線でエラー
  位置は示せる。M9 例題の規模なら不便は小さい。
- 将来 resolver / type checker（I5 / I6）が L2 に接続されたとき、
  **フェーズ境界での多件化** は自然に入る（lex / layout / parse を
  それぞれ 1 件ずつ、resolve / typecheck からも複数件を出す）。
  parser recovery はそれで多くのケースに手が届く。

本トレードオフは I-OQ52 で追跡する。

## 今後の拡張

L2 時点では LSP 側が触れない、けれど近い将来必要になる拡張点を
列挙しておく：

- **I5 / I6 結合**：`AnalysisResult` を `Vec<CompileError>` のまま
  拡張するのではなく、新たに `ResolveError` / `TypeError` を包む
  `SemanticError` を導入し、`analyze` を front-end のみ、新規
  `check` を resolve/typecheck まで含む形にする想定。LSP 側の
  `diagnostics_for` は `check` へ差し替える。
- **incremental parsing**：`Document` に前回の AST と token 列を
  保持し、差分範囲の周辺のみ再 lex/再 parse する。I-OQ9 の解決
  次第。
- **複数 error**：parser recovery が入ると `errors` が 2 件以上
  出るので、`compile_errors_to_diagnostics` の形を変えずにそのまま
  流せる。
- **Quick Fix / Code Action**：`Diagnostic.code` を文字列で予約
  済なので、`CodeActionProvider` を capability に足し、code 別に
  ハンドラを派遣する設計にすれば良い。L6 以降。
- **Hover / goto**：L4 / L5 に入ったとき、`Document` に加えて最後
  に成功した AST と resolver 情報をキャッシュする必要がある。

## 新規 OQ

本タスクで浮上した未決事項：

- **I-OQ52 パーサ error recovery 戦略**：panic mode / error
  productions / FOLLOW set のうちどれを選ぶか。recovery が入ると
  `analyze` が複数 error を返せるようになり、LSP 体験が大きく
  改善する。
- **I-OQ53 UTF-16 変換の最適化 / crate 採用**：現状は自前の
  `LineMap` を毎リクエスト作り直している。インクリメンタル化や
  ホットリロードで coordinate 変換が頻繁になったら、`line-index`
  や rope ベースの実装へ差し替えを検討する。
- **I-OQ54 LSP diagnostic の `relatedInformation` 設計**：型検査
  を繋ぐ段で、unification clash の相方側にも span を付けたく
  なる。I6 と一緒に設計する。
- **I-OQ55 `positionEncoding` negotiation**：LSP 3.17 は UTF-8 /
  UTF-16 / UTF-32 のネゴを許す。VSCode は UTF-16 デフォルトだが、
  サーバ側で UTF-8 宣言すれば変換が不要になる（ただしクライアント
  の対応状況に依存）。将来余裕ができたら検討。
- **I-OQ56 ドキュメントストアの TTL / memory budget**：極端に
  長時間動く LSP プロセスでたくさんのファイルを開閉した場合の
  メモリ蓄積。`didClose` で `remove` しているので通常は問題ない
  が、将来メタデータ（AST キャッシュ等）が増えた段階で再評価。

上記は `docs/open-questions.md` §1.5 に登録済。

## 参照

- `10-lsp-scaffold.md` — L1 の scaffold（本文書が拡張する）
- `07-lsp-stack.md` — LSP stack 選定（`tower-lsp` / `lsp-types`）
- `09-lexer.md` / `13-parser.md` — `LexError` / `ParseError` の形
- `../open-questions.md` §1.5 — I-OQ9 / I-OQ52〜I-OQ56
