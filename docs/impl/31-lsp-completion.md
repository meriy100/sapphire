# 31 LSP completion (L6)

Track L の完了をもって L1..L5 + L7 が揃い、VSCode 拡張としては
診断 / 増分同期 / hover / goto-definition / syntax + snippets が
動いている。L6 はここに `textDocument/completion` を積み、
**識別子入力中にスコープ内の候補を返す** 状態を作る。

本 doc は L6 で採用した候補収集 pipeline と `CompletionItem`
組立ルールを説明する。pipeline 的には L4 hover / L5 goto の
`analyze → resolve (→ typeck)` を流用し、新しい解析層は足さない。

## Scope

### 含む

- `ServerCapabilities::completion_provider` の宣言（trigger
  character は `.` のみ、英数は LSP のデフォルトで発火するので
  明示不要）
- 同一ファイル内の以下を候補として返す：
  - **Local** 束縛（lambda / `let` / pattern / `do`-bind /
    関数パラメータ）
  - **Top-level** 束縛（value / ctor / data / type alias /
    class / class method / `:=`-embed）
  - **Prelude** に由来する `env.unqualified` のエントリ
  - **Module qualifier**（`env.qualified_aliases` の key）
- 修飾 `Qualifier.` 以降の場合は当該モジュールが export する
  名前群に絞る（自モジュールは `env.top_level`、他モジュールは
  `env.unqualified` から module id 一致で引き直す）
- 型スキーム（あれば）を `detail` に付与
- prefix 厳密一致は **しない**。prefix に `startsWith`
  （大文字小文字区別あり）でフィルタし、fuzzy マッチは
  client に委ねる

### 除かない / DEFERRED

- 真の incremental reparse は引き続き `I-OQ9` にぶら下げ。
  全 reparse で L4 / L5 と同じコストで動く。
- **Auto-import 候補**（未 import な識別子の候補化 + import
  追加コード）は I-OQ108 に punt。
- **Snippet completion**（keyword / 制御構文の雛形）は L7 の
  `snippets/` で賄うので重複させない。
- **docstring 表示** は I-OQ98（prelude docstring）側で仕込んだ
  後に `CompletionItem::documentation` に載せる。

## Pipeline

```
Position
  │  (LineMap::byte_offset)
  ▼
byte_offset
  │  (CompletionScan: source を左方向に読む)
  ▼
(qualifier?, prefix)
  │
  ├─ qualifier あり ─► env.qualified_aliases から module id
  │                    → export 名を列挙（自モジュール or unqualified）
  │
  └─ qualifier なし ─► local binders (LocalCollector AST walk)
                       + env.top_level (self module)
                       + env.unqualified (imports / prelude)
                       + env.qualified_aliases.keys() (module 名)

prefix startsWith でフィルタ
  │
  ▼
Vec<CompletionItem> (label / kind / detail)
```

最小仕様として `prefix` が空文字の場合も（カーソルが空白直後
など）スコープ内すべての候補を返す。候補数は M9 規模では高々
数百なので上限 cap は入れない。

## 候補源の優先順位

同名衝突が起きたときのリストアップ順：

1. **Local**（最内 binder から順に）
2. **Top-level**（同一ファイル内の declaration 順）
3. **Unqualified imports**（`env.unqualified` の name 順）
4. **Module qualifier**（`env.qualified_aliases` の key）

表示順は client がアルファベット順などに並べ直すので、
server 側では insertion 順をそのまま返す。

## `CompletionItemKind` 割り当て

| 由来 | kind |
|---|---|
| Local | `VARIABLE` |
| Top-level value / ruby embed | `FUNCTION`（arity 0 も含め
  実用的に `FUNCTION` に寄せる。関数でない単なる値でも
  VSCode の見た目が困らない） |
| Top-level data ctor | `CONSTRUCTOR` |
| Top-level class method | `METHOD` |
| Top-level data type | `CLASS` |
| Top-level type alias | `INTERFACE` |
| Top-level class | `CLASS` |
| Imported / prelude value | `FUNCTION` |
| Imported / prelude ctor | `CONSTRUCTOR` |
| Imported / prelude type | `CLASS` |
| Module qualifier | `MODULE` |

LSP 仕様は `INTERFACE` / `CLASS` を自由に読み替えてよく、
VSCode は kind に応じたアイコンを出すだけなので、
ここでの対応は見た目の揃え方の問題。

## `detail` の組み立て

- `Scheme` が取れる場合：その `Scheme::pretty()` をそのまま
  入れる。
- 取れない場合：kind に応じて `"(local)" / "(data type)" / ...`
  のように hover と同じ言い回しの短い文字列を入れる。

hover 側と揃えた日本語表記（`(prelude)` など）と競合しない
よう、completion では英語の簡潔ラベルだけに絞る。

## trigger character

LSP では client が英数字を打った時点で勝手に completion を
投げる。`trigger_characters` に入れるのは「英数字以外で
completion を発火させたい文字」に限られる。

L6 では **`.` だけ** を指定する。将来：
- Record field 補完（`record.field` 候補化、I-OQ109 候補）
- `import ` のあとの module path 補完

で追加するが、現状ではモジュール修飾と記号補完の区別が
曖昧になるため最小限に留める。

## 現状の未対応と将来

- Auto-import（未 import の名前の候補化 + import 追加
  edit）→ I-OQ108
- Fuzzy match server-side ランキング → 現状 client 任せ。
  I-OQ106 で追跡
- Snippet completion の server 統合 → L7 `snippets/` で既存
- `completion_item.resolve_provider = true` 化と詳細情報の
  遅延ロード → 候補数が増えたときに I-OQ107 で判断
- Module field / record field 補完 → I-OQ109 で trigger
  character 拡張と同時に

## 新 OQ

- **I-OQ106** — `trigger_characters` を `.` のみに絞る判断。
  英数入力は client 側で発火するが、`_` / `'` の扱いは
  client 差がある。実運用で不足が出た段階で拡張。
- **I-OQ107** — Fuzzy vs prefix startsWith の ranking 責務。
  今は prefix のみ。server 側で `filterText` / `sortText` に
  スコアを積んでから、VSCode の built-in fuzzy に委ねるかを
  後続で判断。
- **I-OQ108** — Auto-import completion。未 import 名の候補化と
  `additionalTextEdits` で import 行を追加する機能。Workspace
  scan（I-OQ72）と paired。
- **I-OQ109** — Record field / module field 補完の trigger 拡張。
  `.` の曖昧性を整理したうえで record field access の推論側
  情報が取れた段階で開く。

## 再レビュー時の観点

- `find_completion_items` は pure（I/O なし）か？
- `CompletionItem.label` と `insert_text` の分離は正しいか
  （operator 記号などで insert 不可能なものが混ざらないか）？
- prefix 判定で `module.` 後の `.` を跨いだ scan をしていないか？
