module Main
  ( main )
  where

-- Main action: greet two names in sequence.
main : Ruby {}
main = do
  greet "Sapphire"
  greet "world"

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
