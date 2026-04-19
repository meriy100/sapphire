# frozen_string_literal: true
#
# Generated from module Fetch by sapphire-compiler 0.1.0.
# Targets sapphire-runtime ~> 0.1. Do not edit by hand.
# See docs/build/02-source-and-output-layout.md for the output contract.

require 'sapphire/runtime'
require 'sapphire/prelude'
require 'sapphire/http'

Sapphire::Runtime.require_version!('~> 0.1')

module Sapphire
  class Fetch

    def self.main
      Sapphire::Prelude.monad_bind(((Sapphire::Http.get).call("https://example.com/")), (->(res) { (case (res); in { tag: :Ok, values: [body] }; (Sapphire::Prelude.monad_bind(((Sapphire::Fetch.stringLength).call(body)), (->(n) { (Sapphire::Fetch.rubyPuts).call((("fetched ") + ((((Sapphire::Prelude::SHOW).call(n)) + (" bytes"))))) }))); in { tag: :Err, values: [httpErr] }; ((Sapphire::Fetch.rubyPuts).call((Sapphire::Fetch.explain).call(httpErr))); else; raise 'non-exhaustive case'; end) }))
    end

    def self.explain
      ->(_arg0) { (err = _arg0; (case (err); in { tag: :NetworkError, values: [m] }; ((("network error: ") + (m))); in { tag: :StatusError, values: [c, msg] }; ((("HTTP ") + ((((Sapphire::Prelude::SHOW).call(c)) + (((": ") + (msg))))))); in { tag: :DecodeError, values: [m] }; ((("decode error: ") + (m))); else; raise 'non-exhaustive case'; end)) }
    end

    def self.stringLength
      ->(s) {
          Sapphire::Runtime::Ruby.prim_embed do
            s.bytesize
          end
        }
    end

    def self.rubyPuts
      ->(s) {
          Sapphire::Runtime::Ruby.prim_embed do
            puts s
            {}
          end
        }
    end

    # Entry helper — `sapphire run` dispatches here.
    def self.run_main
      result = Sapphire::Prelude.run_action(main)
      case result[:tag]
      when :Ok
        0
      when :Err
        err = result[:values][0]
        klass = err[:values][0]
        msg   = err[:values][1]
        bt    = err[:values][2]
        warn "[sapphire run] #{klass}: #{msg}"
        bt.each { |line| warn "  #{line}" }
        1
      end
    end
  end
end
