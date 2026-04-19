module Http
  ( get, HttpError(..) )
  where

data HttpError
  = NetworkError String
  | StatusError  Int String
  | DecodeError  String

-- | Fetch a URL, returning the body as a `String` or a classified error.
get : String -> Ruby (Result HttpError String)
get url := """
  require 'net/http'
  require 'uri'

  begin
    uri = URI.parse(url)
    res = Net::HTTP.get_response(uri)
    if res.is_a?(Net::HTTPSuccess)
      { tag: :Ok, values: [res.body] }
    else
      msg = res.message || "unknown"
      { tag: :Err, values: [
        { tag: :StatusError, values: [res.code.to_i, msg] }
      ] }
    end
  rescue => e
    { tag: :Err, values: [
      { tag: :NetworkError, values: [e.message] }
    ] }
  end
"""
