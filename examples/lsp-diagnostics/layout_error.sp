-- examples/lsp-diagnostics/layout_error.sp
--
-- 意図的に **レイアウト段で** 落ちるサンプル。`{` を明示的に開い
-- たまま閉じずに EOF を迎えると、`LayoutErrorKind::
-- UnclosedExplicitBlock` が立つ。
--
-- sapphire-lsp に開かせると `code = sapphire/layout-error` の
-- diagnostic が返る想定。

module Demo where

x = { foo = 1
