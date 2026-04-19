# 23. VSCode extension polish（L7）

Track L の L7 タスクとして、`editors/vscode/` の拡張に
**syntax highlighting / snippets / language configuration /
configuration schema / README 整備** を入れる。L1
（`docs/impl/10-lsp-scaffold.md`）で scaffold した最小 LSP client
に、編集体験の底上げとなる UX 層を被せる。本文書は同じ commit に
載せる設計メモ。

L6（completion）よりも先に出す理由：L6 は I6（typecheck）着地後に
L4 hover / L5 goto-def と組み合わせて真価が出るが、syntax highlight
と snippets は LSP capability に依存しないので前倒しで価値が出せる。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| Grammar 表現 | TextMate grammar JSON（`syntaxes/sapphire.tmLanguage.json`） |
| scope name | `source.sapphire` |
| ブロックコメント nesting | **1 段までの近似**（TextMate の限界） |
| キーワード識別 | spec 02 §Keywords の 20 語を `keyword.control` / `storage.type` に振り分け |
| 三連引用符 | `string.quoted.triple.sapphire` 専用スコープ。`:=` 直後以外にも出る前提 |
| snippets | 13 本（`module` / `main` / `data` / `type` / `case` / `let` / `if` / `do` / `class` / `instance` / `ruby` / `record` / `update`） |
| Configuration schema | `sapphire.lsp.path` / `sapphire.lsp.log` / `sapphire.trace.server` の 3 鍵 |
| 環境変数との優先順 | 環境変数 > 設定 > デフォルト |
| 設定変更時の反映 | 拡張が reload を提案（`vscode.workspace.onDidChangeConfiguration`） |
| icon / marketplace 公開 | 本タスク対象外（D3 で再訪） |

## TextMate grammar の設計方針

Sapphire は off-side rule とネスト可能なブロックコメントを持ち、
TextMate の線形 regex では完全に parse できない。よって grammar は
**近似** に留め、以下を優先している：

1. **キーワードと識別子の明確な分離**。spec 02 §Keywords の 20 語
   はすべて `\b...\b` で拾い、`upper_ident` / `lower_ident` と
   衝突しないようにする。`True` / `False` は `upper_ident` では
   あるが、spec 09 による prelude コンストラクタ扱いを尊重して
   `constant.language.boolean` に昇格させる。
2. **文字列 3 形態の切り分け**。`string_lit`（単引用符 double）と
   `triple_string`（三連引用符）を別スコープにする。triple を先に
   マッチさせることで、`:=` 直後の Ruby 埋め込みが
   `string.quoted.triple.sapphire` として視認できる。
3. **予約パンクチュエーションを個別スコープ化**。`:=` / `->` /
   `<-` / `=>` / `::` / `..` / `\` / `@` は個別の
   `keyword.operator.*` に振り、残りの演算子は fallback の
   `keyword.operator.sapphire` に落とす。
4. **シグネチャ行と Ruby 埋め込み行のヘッド識別子**を
   `entity.name.function.sapphire` に昇格させる。`^\s*lower_ident
   \s*:` / `^\s*lower_ident ... :=` を先頭アンカーで拾う線形 regex
   なので、複数行にまたがる shape は捉えられないが、M9 例題で
   頻出する 1 行シグネチャ・1 行ヘッダは網羅できる。
5. **モジュール名と import を専用スコープ化**。`module Foo.Bar`
   の `Foo.Bar` を `entity.name.namespace.sapphire`、
   `import qualified Foo.Bar as F` の `F` も同様。

### 既知の近似

- **ネストブロックコメント**：TextMate の `begin/end` の中に同じ
  `begin` を再帰 include する形で 1 段だけネストを許している。
  `{- {- {- -} -} -}` のような 2 段以上は hidden。spec 02 §Whitespace
  and comments は完全ネストを許可しているので、規範違反ではなく
  ハイライトの見落としに留まる。
- **`do` ブロックのインデント**：`indentationRules` の正規表現で
  `do` / `of` / `where` / `let` などを trigger としているが、
  spec 02 §Layout のフル挙動（reference column と列比較）は
  再現できていない。深い入れ子では手動インデントが要る。
- **record punning 風の `{ field = ... }`**：`{`/`}` を bracket
  pair に登録しているため auto-close は効くが、record update の
  `{ r | f = v }` と record literal `{ f = v }` の区別は grammar
  では行わない。どちらも `variable.other.sapphire` + `=` の並び
  に見える。
- **infix function の背景色**：`` `foo` `` のような backtick infix
  記法（spec 05 で採否未定）は想定していない。出てきたら grammar
  に `string.quoted.other` を足す。

M9 4 例題（`examples/sources/{01..04}`）と `examples/lsp-smoke/hello.sp`
が見栄えのするハイライトで表示できれば合格ラインとした。semantic
tokens に昇格させるのは I6（typechecker）が着地してから（L7 の
自然な後続タスクとして roadmap に紐付ける）。

## Language configuration の設計

`language-configuration.json` は LSP とは独立して動く層で、
以下を受け持つ：

- **コメント定義**（`--` と `{- -}`）：toggle comment コマンドが
  この設定を引く。block comment は配列で開始／終了を分けて渡す。
- **bracket / auto-close / surrounding pair**：`(`/`[`/`{` の 3 組
  と double quote、ブロックコメント `{- -}` を登録。三連引用符
  `"""` は auto-close 対象にしていない。VSCode は 1 文字目の `"`
  時点で閉じペアを挿入してしまい、連続打鍵で UX が崩れるため。
- **word pattern**：`[A-Za-z_][A-Za-z0-9_']*` で、spec 02
  §Identifiers と揃えている。prime（`'`）が word 境界に含まれる
  ことで、Haskell 風の `foo'` / `foo''` が 1 単語として double
  click 選択できる。
- **indent rule**：`increaseIndentPattern` / `decreaseIndentPattern`
  で簡易 auto-indent。前述の近似どまり。
- **`onEnter` rule**：`--` 行で Enter を打つと次行に `-- ` を自動
  継続する。block comment の継続は VSCode 既定挙動に任せる。

LSP capability（hover / goto-def / completion）が埋まっていなくても、
editor 側でオートクローズやインデント補助が効くようにするのが狙い。

## Snippets 選定基準

`snippets/sapphire.code-snippets` の 13 本は以下の基準で選んだ：

1. **M9 例題の骨格**：`module` + `main : Ruby {}` + `do` + `ruby`
   の 4 本は M9 Example 1〜4 が実際に書く形をそのまま snippet 化。
2. **チュートリアル ch1-5 の主要構造**：`data` / `type` / `case` /
   `let` / `if` / `class` / `instance` / `record` / `update` は
   T1 チュートリアル目次（`docs/tutorial/`）に沿う。
3. **spec 文書の参照を description に明記**：spec 01 / 03 / 04 / 06
   / 07 / 08 / 09 / 11 のどこに規範が載っているかを
   `description` 末尾に入れ、snippet を引くときに仕様書への動線が
   短くなるようにした。

snippets は monotonic extension でよいので、M9 通し実装中に頻出
する形（例：`List` pattern `x::xs`、`Result` の `Ok` / `Err`
パターン matching）があれば後日追加する想定。

## Configuration schema と env vars の優先順位

| Key | Env var | Default |
|---|---|---|
| `sapphire.lsp.path` | `SAPPHIRE_LSP_PATH` | `""`（空なら `$PATH` fallback） |
| `sapphire.lsp.log` | `SAPPHIRE_LSP_LOG` | `"info"` |
| `sapphire.trace.server` | — | `"off"` |

**環境変数 > 設定 > デフォルト** の順で resolve する。理由：

- CLI / launch.json 経由で明示的に env を立てた人（L1 README が
  案内する運用）の指示を壊さない。
- VSCode 設定 UI は上書きが便利で permanent に貼り付きやすい
  ので、環境変数より弱くしておく方が「開発ブランチだけ別 binary」
  のような短期切り替えで競合しにくい。
- 結果、既存 L1 の運用（`SAPPHIRE_LSP_PATH=target/debug/sapphire-lsp`
  を export しての起動）はそのまま動く。

`sapphire.trace.server` は client 側の LSP JSON-RPC トレースで、
`vscode-languageclient` が標準対応する設定 ID（`<section>.trace.server`）
に準拠している。サーバ側（`tracing` で stderr に出すログ）とは別
レイヤなので、両方を独立に on/off できる。

## 設定変更時の反映

`sapphire.*` の 3 鍵はすべて `LanguageClient` 構築時に resolve
するため、実行時の動的変更は効かない。変更を検知したら拡張が
**reload window の提案メッセージ** を出す（無視されれば次回起動
から有効）。`onDidChangeConfiguration` で `affectsConfiguration("sapphire")`
を判定して通知する最小実装。

より高度な動的再起動（`client.restart()` + env 再注入）は将来
検討。現状は userbase が小さいため「reload で OK」に倒した。

## 将来の拡張

- **Semantic tokens**（`textDocument/semanticTokens`）：I6 typechecker
  が着地したら、TextMate の `variable.other.sapphire` を name
  resolution に基づくより正確なスコープに差し替える。`tower-lsp`
  側の実装は L4/L5 と同じ pattern。
- **Inlay hints**：L4 hover の情報源（型推論結果）をそのまま
  `textDocument/inlayHint` に流す。L7 semantic tokens と同時期に
  投入可能。
- **Command palette**：`sapphire.restartServer` / `sapphire.showOutput`
  など。現状は `contributes.commands` を置いていない（最小維持）。
- **Icon asset と marketplace 公開**：D3（初回 release）で扱う。
  現 tree には PNG を置かず、`package.json` の `icon` フィールドも
  未設定。`vsce package` は可能だが publish はしない方針。

## 新規 OQ

以下を `docs/open-questions.md` §1.5 に追加する（いずれも
`DEFERRED-IMPL`）。

- **I-OQ76** TextMate grammar の射程とネストブロックコメントの
  扱い。1 段ネスト止まりを許容するか、tree-sitter ベースの
  `vscode-textmate` 拡張 / LSP semantic tokens に昇格させるかは
  I6 着地後に再評価。
- **I-OQ77** Language configuration の `indentationRules` の精度。
  spec 02 §Layout のフル挙動を正規表現で模倣しているが、深い
  入れ子では破綻する。代替は LSP formatting provider / semantic
  indent hint。
- **I-OQ78** 拡張の marketplace 公開タイミングと `publisher` 名
  確定。D3（初回 release）で user に最終確認。現状 `publisher:
  "meriy100"` は placeholder。
- **I-OQ79** 設定変更時の反映ポリシー（reload 誘導 vs 動的
  `client.restart()`）。現状 reload のみ。環境変数経由で頻繁に
  切り替える開発ルーチンが主流になれば動的化を検討。

## 参照

- `07-lsp-stack.md` — L0 の crate 選定
- `10-lsp-scaffold.md` — L1 scaffold（本文書の前提）
- `17-lsp-diagnostics.md` — L2 の LSP diagnostics 導入
- `21-lsp-incremental-sync.md` — L3 の incremental sync
- `06-implementation-roadmap.md` — Track L の L1〜L7 位置付け
- `../spec/02-lexical-syntax.md` — 予約語・識別子・文字列・演算子
- `../spec/10-ruby-interop.md` — `:=` と triple-quoted string
