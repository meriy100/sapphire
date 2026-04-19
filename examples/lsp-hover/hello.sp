-- examples/lsp-hover/hello.sp
--
-- L4 hover (textDocument/hover) 動作確認用サンプル。
-- sapphire-vscode 拡張を Extension Host で起動し、このファイルを開いて
-- 識別子の上にキャレットを置くと、Markdown ツールチップで
-- 型スキームが popup される。
--
-- 動作確認できる hover パターン：
--
--   * 関数シグネチャ / 本体での **top-level value** 参照
--     - `main` 本体の `greet` → `greet : String -> Ruby {}`
--     - `greet` 本体の `makeMessage` / `rubyPuts` も同じ
--   * **constructor** 参照
--     - `pick` 本体の case-arm `A` / `B` → `A : T` / `B : T`
--     - `packHalf` の `Just` → Prelude constructor
--   * 型位置の **data type** / **alias**
--     - `pick : T -> Int` の `T` → `(data type)` タグ付き
--     - `greet : String -> Ruby {}` の `String` / `Ruby` → prelude
--   * **local** 束縛（名前のみ表示、型は I-OQ96 で I6 拡張後）
--     - `greet name = ...` の右辺 `name` → `(local)`
--     - `makeMessage` 内 `let greeting = ...` の参照 → `(local)`
--   * **Ruby-embedded binding** (`:=`)
--     - `rubyPuts` 呼び出し位置で `rubyPuts : String -> Ruby {}`
--
-- 期待される hover 表示（抜粋）:
--
--   - `greet` にカーソルを置くと：
--       ```sapphire
--       greet : String -> Ruby {}
--       ```
--       _(top-level value)_
--   - `Just` にカーソルを置くと：
--       ```sapphire
--       Just : forall a. a -> Maybe a
--       ```
--       _(prelude)_
--   - `name`（`greet` の右辺）にカーソルを置くと：
--       ```sapphire
--       name
--       ```
--       _(local)_
--       _型情報未取得_

module Main
  ( main )
  where

-- Main action: greet using the Ruby monad.
main : Ruby {}
main = do
  greet "Sapphire"

-- A pure function produces the greeting string.
greet : String -> Ruby {}
greet name = rubyPuts (makeMessage name)

-- Pure Sapphire builds the message, no Ruby involved.
makeMessage : String -> String
makeMessage name =
  let greeting = "Hello, "
  in greeting ++ name ++ "!"

-- A data type plus a pattern-matching consumer, for ctor-level hover.
data T = A | B

pick : T -> Int
pick t = case t of
  A -> 1
  B -> 2

-- Prelude ctor reference — hover must tag it as `(prelude)`.
packHalf : Int -> Maybe Int
packHalf n = Just n

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
