-- examples/lsp-incremental/hello.sp
--
-- L3 の incremental document sync 確認用サンプル。VSCode の
-- sapphire-vscode 拡張から開くと language id が `sapphire` になり、
-- sapphire-lsp が子プロセスとして起動する。既定では
-- TextDocumentSyncKind::Incremental を宣言しており、1 文字ずつの
-- 編集に対しても client は差分（range-based change）だけを送る。
--
-- 動作を目で確かめる手順は `README.md` を参照。壊す → 戻す を
-- 連続で行ったとき赤下線がその場で出入りすれば incremental 経路が
-- 機能している。
--
-- 中身は `examples/lsp-smoke/hello.sp` と同等の hello world。
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
makeMessage name = "Hello, " ++ name ++ "!"

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
