-- examples/lsp-goto/hello.sp
--
-- L5 の goto-definition 確認用サンプル。VSCode の sapphire-vscode
-- 拡張を Extension Host で起動したあと、このファイルを開いて
-- 識別子の上で F12（Go to Definition）を叩くと、同じファイル内の
-- 定義位置へ飛ぶ。
--
-- 動作確認できる goto パターン：
--
--   * 関数間参照
--     - `main` の本体で `greet` → `greet` 宣言行
--     - `greet` の本体で `rubyPuts` / `makeMessage` → 同名宣言行
--   * 型シグネチャ内の型名参照
--     - `greet : String -> ...` の `String` → Prelude → goto なし
--     - `main : Ruby {}` の `Ruby` → Prelude → goto なし
--   * データ型と constructor
--     - `pick` の case-arm `A` / `B` → `data T` の constructor 定義
--     - `pickSig : T -> ...` の `T` → `data T` の型名位置
--   * 同一関数内の局所束縛
--     - `greet name = rubyPuts (makeMessage name)` の `name`
--     - let 束縛 `let local = ...` の `local` 参照
--
-- L5 現状の制約：
--
--   * 同一ファイル内の定義にのみ飛べる（import 先 module の定義へは
--     飛ばない。I-OQ72 で記録）。
--   * Prelude の名前（`+`, `map`, `Int` など）にカーソルを置いても
--     定義位置は無い（Prelude は静的テーブルで、.sp ファイルが
--     存在しない）。I-OQ73。
--   * resolver が失敗するソース（未定義名を含む等）では goto 自体
--     動かなくなる。本 L5 は resolver success 前提。

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

-- A data type plus a pattern-matching consumer, for ctor-level goto.
data T = A | B

pick : T -> Int
pick t = case t of
  A -> 1
  B -> 2

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
