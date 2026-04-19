-- examples/lsp-smoke/hello.sp
--
-- L1 の smoke 用 .sp ファイル。VSCode の sapphire-vscode 拡張を
-- Extension Host で起動したあと、このファイルを開くと language id
-- が `sapphire` になり、sapphire-lsp が子プロセスとして起動する。
--
-- L1 時点では initialize / shutdown / textDocument sync の受信
-- しか走らないので、構文が正しく解釈されるわけではない点に注意。
-- パーサ接続は L2 以降の Track L で行う。
--
-- 中身は docs/spec/12-example-programs.md の "hello world" 例を
-- そのまま引いている。
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
