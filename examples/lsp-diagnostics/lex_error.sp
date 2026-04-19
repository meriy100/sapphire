-- examples/lsp-diagnostics/lex_error.sp
--
-- 意図的に **レキサ段で** 落ちるサンプル。spec 02 §Identifiers は
-- 識別子の開始文字を ASCII に限定しているため、以下の `αβ` は
-- `LexErrorKind::NonAsciiIdentStart` で弾かれる。
--
-- sapphire-lsp に開かせると、エディタ側で赤下線 + `code =
-- sapphire/lex-error` の diagnostic が返る想定。

module Demo where

αβ = 1
