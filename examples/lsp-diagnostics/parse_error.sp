-- examples/lsp-diagnostics/parse_error.sp
--
-- 意図的に **パーサ段で** 落ちるサンプル。`data T` に `=` が続か
-- ずに終端するため、`ParseErrorKind::UnexpectedEof { expected: "=" }`
-- （または相当する `Expected { expected: "=", ... }`）が立つ。
--
-- sapphire-lsp に開かせると `code = sapphire/parse-error` の
-- diagnostic が返る想定。

module Demo where

data T
