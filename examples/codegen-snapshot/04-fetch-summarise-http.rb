# frozen_string_literal: true
#
# Generated from module Http by sapphire-compiler 0.0.0.
# Targets sapphire-runtime ~> 0.1. Do not edit by hand.
# See docs/build/02-source-and-output-layout.md for the output contract.

require 'sapphire/runtime'
require 'sapphire/prelude'

Sapphire::Runtime.require_version!('~> 0.1')

module Sapphire
  class Http

    Sapphire::Runtime::ADT.define_variants(self, [[:NetworkError, 1], [:StatusError, 2], [:DecodeError, 1]])

    def self.get
      ->(url) {
          Sapphire::Runtime::Ruby.prim_embed do
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
          end
        }
    end
  end
end
