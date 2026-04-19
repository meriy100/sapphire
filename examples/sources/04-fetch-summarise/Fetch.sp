module Fetch
  ( main )
  where

import Http (get, HttpError(..))

main : Ruby {}
main = do
  res <- get "https://example.com/"
  case res of
    Ok body -> do
      n <- stringLength body
      rubyPuts ("fetched " ++ show n ++ " bytes")
    Err httpErr -> rubyPuts (explain httpErr)

explain : HttpError -> String
explain err = case err of
  NetworkError m     -> "network error: " ++ m
  StatusError  c msg -> "HTTP " ++ show c ++ ": " ++ msg
  DecodeError  m     -> "decode error: " ++ m

-- Ruby bridge: ask Ruby for the string's byte length.
-- 09's prelude does not (yet) ship String-length; the Ruby side
-- handles it here.
stringLength : String -> Ruby Int
stringLength s := """
  s.bytesize
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
