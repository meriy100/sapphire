-- examples/lsp-diagnostics/good.sp
--
-- パースが通る最小の Sapphire モジュール。sapphire-lsp に繋いで
-- 開いても diagnostic は一切出ない想定。L2 の "エラーが無いときに
-- エディタが静かであること" を確認するための基準サンプル。

module Demo (greeting) where

-- 純粋な値。signature と value def のペアは M9 例題でも頻出する
-- トップレベル形で、パーサ / レイアウト / レキサのいずれも通る。
greeting : String
greeting = "hello, sapphire"
