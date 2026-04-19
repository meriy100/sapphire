# frozen_string_literal: true
#
# Generated from module NumberSum by sapphire-compiler 0.1.0.
# Targets sapphire-runtime ~> 0.1. Do not edit by hand.
# See docs/build/02-source-and-output-layout.md for the output contract.

require 'sapphire/runtime'
require 'sapphire/prelude'

Sapphire::Runtime.require_version!('~> 0.1')

module Sapphire
  class NumberSum

    def self.main
      Sapphire::Prelude.monad_bind(((Sapphire::NumberSum.rubyReadLines).call("numbers.txt")), (->(raw) { (case ((Sapphire::NumberSum.parseAll).call(raw)); in { tag: :Ok, values: [ns] }; ((Sapphire::NumberSum.rubyPuts).call((Sapphire::Prelude::SHOW).call((Sapphire::NumberSum.sumOf).call(ns)))); in { tag: :Err, values: [e] }; ((Sapphire::NumberSum.rubyPuts).call((("parse failed: ") + (e)))); else; raise 'non-exhaustive case'; end) }))
    end

    def self.parseAll
      ->(_arg0) { (case [_arg0]; in [[]]; (Sapphire::Runtime::ADT.make(:Ok, [[]])); in [[s, *ss]]; (Sapphire::Prelude.monad_bind(((Sapphire::NumberSum.parseInt).call(s)), (->(n) { Sapphire::Prelude.monad_bind(((Sapphire::NumberSum.parseAll).call(ss)), (->(ns) { Sapphire::Runtime::ADT.make(:Ok, [[n, *ns]]) })) }))); else; raise 'non-exhaustive function clauses'; end) }
    end

    def self.parseInt
      ->(_arg0) { (s = _arg0; (case ((Sapphire::Prelude::READ_INT).call(s)); in { tag: :Nothing, values: [] }; (Sapphire::Runtime::ADT.make(:Err, [(("not an integer: ") + (s))])); in { tag: :Just, values: [n] }; (Sapphire::Runtime::ADT.make(:Ok, [n])); else; raise 'non-exhaustive case'; end)) }
    end

    def self.sumOf
      ((Sapphire::Prelude::FOLDL).call(Sapphire::Prelude::OP_PLUS)).call(0)
    end

    def self.rubyReadLines
      ->(path) {
          Sapphire::Runtime::Ruby.prim_embed do
            File.readlines(path).map(&:chomp)
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
