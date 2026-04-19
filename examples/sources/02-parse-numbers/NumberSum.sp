module NumberSum
  ( main )
  where

-- | Read a file, parse integers per line, sum them, print the result.
main : Ruby {}
main = do
  raw <- rubyReadLines "numbers.txt"
  case parseAll raw of
    Ok ns  -> rubyPuts (show (sumOf ns))
    Err e  -> rubyPuts ("parse failed: " ++ e)

-- Parse a list of strings into a list of ints, failing fast on any
-- non-integer line. Pure `Result`-monadic.
parseAll : List String -> Result String (List Int)
parseAll []       = Ok []
parseAll (s::ss)  = do
  n  <- parseInt s
  ns <- parseAll ss
  pure (Cons n ns)

-- Pure parse of a single string. Uses `readInt` from 09's
-- utility set (added 2026-04-18).
parseInt : String -> Result String Int
parseInt s = case readInt s of
  Nothing -> Err ("not an integer: " ++ s)
  Just n  -> Ok n

-- Fold a list of ints.
sumOf : List Int -> Int
sumOf = foldl (+) 0

-- Ruby bridge: read a file as a list of chomped lines.
rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
