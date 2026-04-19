-- examples/lsp-completion/hello.sp
--
-- L6 completion (textDocument/completion) 動作確認用サンプル。
-- sapphire-vscode 拡張を Extension Host で起動し、このファイルを開いて
-- Ctrl+Space（もしくは識別子途中の自動 popup）を叩くと、スコープ
-- 内の識別子候補が Markdown 付きで popup される。
--
-- 動作確認できる completion パターン：
--
--   * **Top-level value 候補**
--     - `main` 本体で `gr` まで打つと `greet`, `greeting` が候補化
--     - kind は FUNCTION、detail には推論済み scheme が乗る
--   * **Local binder 候補**
--     - `makeMessage` 本体の `let greeting = ...` 以降で `gree`
--       まで打つと `greeting`（local）と `greet`（top-level）
--       が両方出る。local の detail は `(local)`
--   * **Prelude 候補**
--     - `main` で `ma` → `map`（Prelude）、`Ju` → `Just`（prelude
--       constructor）
--   * **Module qualifier**
--     - `M.` まで打つと自モジュール内の top-level が候補化
--     - 上級：別モジュールを `import Foo as F` したときに
--       `F.` で Foo の export が出る（本サンプルでは import は
--       Prelude のみ）
--   * **Constructor 候補**
--     - `data T = Alpha | Beta` の use 位置で `Al` → `Alpha`
--       （kind は CONSTRUCTOR）
--
-- 期待される候補表示（抜粋）:
--
--   - `main = gr` の `gr` 直後で：
--       greet        (FUNCTION) — `String -> Ruby {}`
--       greeting     (FUNCTION) — `String`
--   - `makeMessage` 内 `in gree` の直後で：
--       greeting     (VARIABLE) — `(local)`
--       greet        (FUNCTION) — `String -> Ruby {}`
--   - `main = Ju` の直後で：
--       Just         (CONSTRUCTOR) — `forall a. a -> Maybe a`

module Main
  ( main )
  where

-- Top-level greeting (value).
greeting : String
greeting = "Hello, "

-- Main action: greet using the Ruby monad.
main : Ruby {}
main = do
  greet "Sapphire"

-- Pure function with a shadow between top-level `greeting` and the
-- local `greeting` in `makeMessage` — completion should surface both
-- distinctly (top-level FUNCTION / local VARIABLE).
greet : String -> Ruby {}
greet name = rubyPuts (makeMessage name)

makeMessage : String -> String
makeMessage name =
  let greeting = "Hi there, "
  in greeting ++ name ++ "!"

-- A data type plus a pattern-matching consumer, for ctor-level
-- completion.
data T = Alpha | Beta

pick : T -> Int
pick t = case t of
  Alpha -> 1
  Beta  -> 2

-- Prelude ctor reference — completion on `Ju` should surface Just
-- with CONSTRUCTOR kind.
packHalf : Int -> Maybe Int
packHalf n = Just n

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
