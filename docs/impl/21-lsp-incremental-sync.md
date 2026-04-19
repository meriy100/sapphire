# 21. LSP incremental document sync（L3）

Track L の L3 タスクとして、`sapphire-lsp` サーバの text sync 経路を
**Full（毎回全文）** から **Incremental（range-based な差分）** に
切り替える。本文書は実装と同じ commit に載せる設計メモで、
L2（`docs/impl/17-lsp-diagnostics.md`）が置いたドキュメントストアと
解析ドライバを土台に「差分の受け取り方」だけを変える。

L4（hover）/ L5（goto-definition）/ L6（completion）で AST / 解析
結果のキャッシュを `Document` に載せるときに、本 L3 が決めた
**"text は incremental、解析は full"** の分離が効く想定。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| `ServerCapabilities::text_document_sync` | **`TextDocumentSyncKind::Incremental`**（L2 の `Full` から upgrade） |
| 差分適用 | `crate::edit::apply_change(&mut String, &TextDocumentContentChangeEvent)`。UTF-16 `Range` を byte offset に変換し `String::replace_range` で置換。 |
| `range == None` のケース | 全置換（LSP 仕様上は Incremental 宣言時も許される fallback）。同じ関数で吸収。 |
| `rangeLength` フィールド | **無視**。LSP 3.17 で deprecated、`range` が authoritative。 |
| UTF-16 → byte 変換 | `LineMap::byte_offset(Position) -> Option<usize>`（L2 の `position(byte) -> Position` の逆）。char 境界に snap して返す。 |
| LineMap の更新 | 各 change 適用ごとに **毎回再構築**（最小実装）。真の incremental LineMap は I-OQ9 / I-OQ53 で継続 punt。 |
| 再解析モデル | **full reparse** を維持。edit のたびに `analyze(text)` を丸ごと呼ぶ。 |
| Version 単調性 | `refresh_incremental` 内で stored `version` が incoming 以上なら drop。`debug_assert` で post-insert 一致を確認。 |
| Apply エラー | `ApplyError { StartOutOfRange / EndOutOfRange / InvertedRange }`。最初のエラーでバッチ中止（既に適用した change は残す）。 |
| race 対策 | L2 と同形：「insert → analyze → version 確認 → publish」を `refresh_incremental` / `refresh_full` で共用 (`analyze_and_publish`)。 |

## Incremental を採用する理由

1. **差分サイズが一定**：client は 1 キー押下あたり数バイトの
   change を送るだけになる。Full sync では毎回 "ファイル全体"
   が JSON-RPC frame に載るので、数百行のソースでも 1 文字ごと
   に kB オーダの payload が飛ぶ。L3 以降で巨大ファイルを扱う
   余地を確保するため、capability を早めに upgrade しておく。
2. **VSCode client の native 挙動**：VSCode は server が
   `Incremental` を宣言すれば自動的に range-based change を
   送る。TypeScript extension 側に手を入れる必要がないので、
   L1 の `editors/vscode/` 設定をそのまま使える。
3. **Full fallback を失わない**：LSP は Incremental 宣言時でも
   `range == None` の change（全置換）を送っていい余地を残す。
   `apply_change` が両方を 1 本で扱うので、client 実装のばら
   つきで壊れない。

## Why split text-sync from reparse

L2 で「full reparse を維持する」と punt した判断（`docs/impl/
17-lsp-diagnostics.md` §なぜ full reparse か）を、L3 でも引き
継ぐ：

- **テキスト差分 ≠ AST 差分**。range-based edit を受けたからと
  いって「影響範囲の AST ノードだけ再パース」が直接できるわけ
  ではない。AST ノードの永続化、依存追跡、再開可能な lexer /
  parser 状態 — いずれも L3 のスコープ外。
- **レイヤ分離で将来を縛らない**。text sync を incremental に
  する一方、下流の解析はそのまま full のままにしておけば、
  後段で Salsa 風 memo / tree-sitter 風 reparse / rope ベース
  の buffer のどれに振っても L3 の差分適用コードは使い回せる。
- **M9 例題規模で full reparse が痛くない**：数百行のモジュール
  に対して `analyze` 一発は数 ms オーダー。edit 側のレイテンシ
  感は差分適用が決め、解析レイテンシは L4/L5 の AST キャッシュ
  導入時に局所化すれば良い。

したがって本 L3 の範囲は **「`did_change` のバッファ更新方法を
差分型にする」** のみに狭める。reparse の挙動は L2 と同一で、
`apply_change` 適用後の完成バッファに対して `analyze` を 1 回
呼ぶ。

## UTF-16 → byte 変換の実装方針

LSP 3.17 `Position` は UTF-16 code unit（`positionEncoding` を
negotiate していないので default、`docs/impl/17-lsp-diagnostics.md`
§LSP UTF-16 と byte offset の変換）。L3 で新しく必要になった
"Position → byte offset" は、L2 の `position(byte) -> Position` の
**ほぼ逆演算**を同じ `LineMap` に生やす：

```rust
fn byte_offset(&self, pos: Position) -> Option<usize>
```

- `pos.line` が `line_starts.len()` を超えていれば `None`。
- そのラインの範囲 `[line_start, line_end_excl)` を取り
  （`line_end_excl` は次の行の `\n` / `\r\n` を除いた位置）、
  `pos.character` 分の UTF-16 code unit を先頭から消費して
  byte offset を進める。
- サロゲートペアの途中を指された場合は **コードポイントを
  分割せず、直前の char 境界に snap** して返す。これにより
  `String::replace_range` に安全な byte 境界だけを渡せる。
- `pos.character` が行末（改行除き）を超えた場合は **clamp**
  して行末 byte を返す。LSP の一部 client（特に削除系の
  change）が "line_length" を column に載せることがあるため。

`LineMap` 自体は L2 の実装を触らず、`byte_offset` だけを足す形で
拡張した。`utf16_len` は L3 で他モジュール（incremental sync の
ドキュメント、将来の hover）と共有するため `pub` に昇格させた。

## Apply の形と error handling

`edit::apply_change` は **pure function** で、引数 `buf: &mut
String` に対して 1 件の `TextDocumentContentChangeEvent` を適用
する：

```rust
pub fn apply_change(buf: &mut String, change: &TextDocumentContentChangeEvent)
    -> Result<(), ApplyError>;
```

- `change.range == None` → `buf` を `change.text` で全置換。
- `change.range == Some(r)` → `LineMap::new(buf)` を作り
  `r.start`/`r.end` を byte offset に変換、`String::replace_range`
  で置換。
- LineMap は関数内で毎回作る。**複数 change を 1 バッチで送る
  LSP の契約上、change ごとに buffer は変わる**ので、caller が
  LineMap を使い回してはいけない。

### range_length を無視する判断

LSP 3.17 §DidChangeTextDocumentParams で `rangeLength` は
**deprecated**、かつ実装が UTF-16 で数えるか byte で数えるか
割れている（TypeScript lang server は UTF-16、rust-analyzer は
byte）。`range` を authoritative と見る方が壊れにくく、ecosystem
も同じ方向に倒している。Sapphire では `range_length` を読まず、
`range` だけで決める。新 OQ I-OQ68 として記録しておく。

### エラー処理: 最初のエラーで抜ける

`apply_change` が返しうる `ApplyError`：

- `StartOutOfRange` — `range.start` が現バッファ外
- `EndOutOfRange` — `range.end` が現バッファ外
- `InvertedRange` — `start > end`

`refresh_incremental` は change を順に適用し、**最初のエラーで
バッチを止めて残りを捨てる**。選択肢は 3 つあった：

1. エラーを pass する（残り change も試す）
2. エラーを log して batch を止める（**採用**）
3. エラーで `did_change` 全体を捨てる（＝以前の buffer を残す）

2 を採った理由：LSP 仕様上、バッチ内 change は **前の change が
適用された前提で組み立てられる** ため、1 本失敗したあとに後続を
走らせても座標系がズレて無意味な操作になる。3 は client 側と
server 側の buffer が乖離したまま "何事も無かったかのように"
見せるので、次の `didChange` がさらに壊れやすい。2 なら
server ログに WARN が出て、client 次第で `textDocument/didOpen`
を再送してリセットできる（将来 `workspace/diagnostic/refresh` や
`textDocument/willSaveWaitUntil` を足すときの resync フックに
なる）。

新 OQ I-OQ69 として、resync プロトコル（client に再送を促す）を
将来検討する余地を残した。

## Version 単調性の保証

LSP 3.17 §TextDocumentItem は **clients must send strictly
increasing versions on `textDocument/didChange`**。server 側は
race でこれを破ってはいけない。本 L3 では：

```rust
async fn refresh_incremental(&self, uri, changes, version) {
    let starting_text = match self.documents.get(&uri) {
        Some(e) if e.version >= version => return, // 古い or 同値は drop
        Some(e) => e.text.clone(),
        None    => String::new(),
    };
    let (new_text, err) = apply_changes(&starting_text, &changes);
    self.documents.insert(uri, Document { text: new_text.clone(), version });
    debug_assert!(store.version == version);
    self.analyze_and_publish(uri, new_text, version).await;
}
```

ポイント：

- **insert は analyze の前**。concurrent な新しい `did_change` が
  入って来ても、自分より先に覗いたときに必ず `>= version` を
  見て自分を捨てる。
- **publish の前に store を再確認**（`analyze_and_publish`）。
  自分が analyze している間に newer version が insert されたら
  publish を skip する。LSP は "latest wins" なので newer 側が
  自分の publish を置き直す。
- `debug_assert!` で post-insert の一致を確認。concurrent 経路
  の race は debug build で早期に気付ける。

`did_open` 経路（`refresh_full`）には monotonicity 制約を課さ
ない：open は無条件上書きが LSP の正しい挙動。

## `did_close` の扱い

L2 の挙動をそのまま残す。`documents.remove` → 空 diagnostic を
publish して squiggle をクリア。

## LineMap の差分更新を punt する理由

`apply_change` は毎回 `LineMap::new(buf)` を作る。**edit 後の
LineMap 再構築** と **edit 前の LineMap から差分更新** の 2 択
のうち、本 L3 は前者を採る：

- M9 例題規模（数百行）で 1 回の LineMap 構築は μs オーダー。
  edit キー 1 つあたりの overhead が人体感知閾値に乗らない。
- 差分更新は「挿入された text の `\n` 数 / 削除された region の
  `\n` 数」を見て `line_starts` を slice で合成する素朴形。
  ロジックは書けるが、**テスト面の複雑さ**（CRLF 境界、0/1
  change の corner case、複数 change のバッチ内での map の
  生存期間）が増える。L3 のゴールは capability 切り替えなので、
  map 更新の最適化はスコープを跨がせない。
- 将来 rope / tree-sitter に乗せ替える時に LineMap 自体が消える
  可能性がある。早すぎる最適化を避ける。

これは I-OQ53（UTF-16 変換の最適化 / crate 採用）と I-OQ9
（incremental 計算基盤）の延長線で扱う。新 OQ I-OQ67 として
「LineMap の部分更新」を明示的に切り出して記録する。

## race-free refresh の形（L2 との差分）

L2 の `refresh` を L3 で 2 本に分けた：

| 関数 | 使う notification | starting buffer | 差分 |
|---|---|---|---|
| `refresh_full` | `did_open` | n/a（params.text） | 無条件上書き |
| `refresh_incremental` | `did_change` | `documents.get(&uri)` | version 単調性 chek、`apply_changes` |
| `analyze_and_publish` | 両方 | — | analyze 実行と publish race guard |

`analyze_and_publish` が共通化されたことで、L2 と L3 の race 保護
ロジックが重複せず、将来の L4/L5 で他 notification（`did_save`
など）を足すときも同じ guard を使い回せる。

## テスト戦略（L3 スコープ）

- `edit::tests` — 18 本前後。insert / delete / replace、zero-width
  range、multi-byte 境界、CRLF 跨ぎ、複数 change 連続適用、range
  out-of-bounds、`range_length` 無視、など。
- `diagnostics::tests` — 既存 L2 の LineMap テストに加え、
  `byte_offset` の round-trip（position → byte → position）、
  surrogate 境界での snap、行末 clamp、OOB で `None`、空ソース。
- `server::tests` — L2 の diagnostic smoke に加え、
  `apply_changes` 連続適用 / 全置換と range 混在 / エラー中止
  の 4 本。
- Integration は `examples/lsp-incremental/` の README に手順を
  置き、VSCode 実機確認は user 側で行う。L2 の `example_
  diagnostics.rs` と同形の integration test は本 L3 では追加
  しない（差分適用は unit test で尽きる）。

合計で **20 本以上**（本タスクの acceptance）を満たす。

## L4 / L5 への引き継ぎ

L3 の成果物が L4（hover）/ L5（goto-def）で効くポイント：

- **`Document` に AST / resolver 結果を足す余地**。`Document`
  struct は現在 `{ text, version }` のみ。L4 で hover 情報を
  引くときは `Document { text, version, analysis: Option<
  AnalysisResult> }` のように拡張する。`refresh_incremental`
  が最後の analyze 結果を保持し、capability handler は store
  から読むだけになる。
- **version-tagged cache**。LSP の hover / goto request は
  `TextDocumentIdentifier` を持ち、version tag は付かない。
  本 L3 の monotonic version で「cache の鮮度」を
  `documents.get(uri).version` で取れる仕組みが既にある。
- **incremental 化への段階的 upgrade 余地**。現状は
  `apply_change` が毎回 `LineMap::new` を呼んでいるが、後々
  `Document` に `line_map: LineMap` を保持し、`apply_change`
  の after で差分更新するだけで済むレイアウト。I-OQ67 を
  起こすときに同時に整理する。
- **`analyze_and_publish` の拡張**。hover / goto は publish で
  はなく request-response なので、同じ race guard
  （`version` 一致確認）を "最新 analyze が完了するまで待つ /
  stale 結果を拒否する" の形で再利用できる。

## 新規 OQ

本タスクで浮上した未決事項：

- **I-OQ67 LineMap の部分更新**：現状 edit ごとに `LineMap::new`
  を呼び直す。巨大ファイル / 高頻度編集で hot path になるなら、
  `line_starts` を slice で継ぎ合わせる incremental 版に差し
  替える。`line-index` / `ropey` を採用する判断と連動（I-OQ53）。
- **I-OQ68 `rangeLength` の取り扱い**：現状は無視。一部クライ
  アントが `range` と矛盾する `rangeLength` を送ってきた時の
  診断（例："client 側 UTF-16 算出が壊れている" を log で可視
  化する）が必要になるか、様子を見る。
- **I-OQ69 client 再送 (resync) プロトコル**：`apply_change`
  エラーで buffer が drift したとき、`workspace/diagnostic/
  refresh` / 明示的な `textDocument/didOpen` 再送で client に
  フル再送を促すフック。LSP 3.17 に既存の mechanism が揃って
  いるかを確認してから決める。
- **I-OQ9 は継続 DEFERRED-IMPL**：text sync は incremental に
  なったが、reparse / AST 再利用の incremental 化はまだ。
  Salsa 導入 / `lsp-server` への乗せ替え判断は先送り。

## 参照

- `17-lsp-diagnostics.md` — L2 の diagnostics と UTF-16 変換の
  設計（`LineMap` / `utf16_len` の実装）
- `10-lsp-scaffold.md` — L1 scaffold
- `07-lsp-stack.md` — `tower-lsp` スタック選定
- `06-implementation-roadmap.md` — Track L の L3 位置付け
- `../open-questions.md` §1.5 — I-OQ9 / I-OQ52〜I-OQ56 / I-OQ67〜I-OQ69
