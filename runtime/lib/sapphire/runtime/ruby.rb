# frozen_string_literal: true

module Sapphire
  module Runtime
    # The `Ruby` effect-monad evaluator.
    #
    # Per docs/spec/11-ruby-monad.md and
    # docs/build/03-sapphire-runtime.md §The `Ruby` monad evaluator,
    # this module exposes the three primitives the generated code
    # invokes:
    #
    # - `prim_return(value)`  — produces a `Ruby a` action whose
    #                           execution yields `value` immediately
    #                           (spec 11 §Primitives `primReturn`).
    # - `prim_bind(action, &k)` — sequentially composes a `Ruby a`
    #                             with a continuation (spec 11
    #                             `primBind`).
    # - `prim_embed(&body)`   — wraps a Ruby-side `Proc` (the
    #                           compiled form of a `:=`-bound
    #                           snippet per docs/build/03 §`:=`-bound
    #                           snippet entry) as a `Ruby a` action.
    # - `run(action)`         — drives an action to completion,
    #                           returning a `Result RubyError a`-
    #                           shaped Sapphire value (spec 11 §run).
    #
    # ## Cross-reference to spec naming
    #
    # Spec 11 names the primitives in camelCase (`primReturn`,
    # `primBind`) since that is the Sapphire surface syntax
    # introduced by that document. The Ruby-side evaluator uses
    # snake_case (`prim_return`, `prim_bind`, `prim_embed`) per
    # Ruby naming convention. The two name sets are in one-to-one
    # correspondence; generated code (I7c) bridges them.
    #
    # ## Opaque action values
    #
    # A `Ruby a` value on the Ruby side is an instance of the
    # nested class `Action` (below). Per spec 11 §Type signature
    # the type is opaque: users do not pattern-match on it, the
    # ADT helpers do not treat it as a tagged ADT
    # (`ADT.tagged?(action)` returns false), and the marshal
    # helpers refuse to move it across the boundary — `run` is the
    # only exit (spec 11 §There is no `unsafeRun` / `runIO`).
    #
    # ## Execution model
    #
    # Per spec 11 §Execution model the default execution model is
    # **single-threaded on the Ruby side**: each `>>=` step fully
    # completes before the next begins. This R4 implementation
    # evaluates actions synchronously on the caller's thread — a
    # deliberate simplification. Spawning a dedicated Ruby
    # evaluator thread (so that `run` blocks the Sapphire caller
    # on a distinct OS thread) is R5's responsibility per
    # docs/impl/06-implementation-roadmap.md §Track R. The
    # observable semantics here already satisfy spec 11 §Execution
    # model items 1-5 because no sub-step can observe that the
    # evaluator thread is in fact the caller thread.
    #
    # There is deliberately no `unsafeRun` / escape hatch (spec 11
    # §There is no `unsafeRun` / `runIO`).
    module Ruby
      # Opaque action wrapper.
      #
      # An `Action` carries one of three discriminated payloads,
      # expressed via a `kind` symbol:
      #
      # - `:pure`   — holds a Sapphire-ready `value`. `prim_return`.
      # - `:embed`  — holds a `Proc` that, when called with zero
      #               arguments, runs Ruby-side code and returns a
      #               Ruby-side value to be marshalled back to
      #               Sapphire. `prim_embed`.
      # - `:bind`   — holds an upstream action `ma` and a
      #               continuation `k : a -> Ruby b`. `prim_bind`.
      #
      # Action instances are **frozen** after construction. Their
      # internal representation is private: generated code and
      # user code must go through `prim_return` / `prim_embed` /
      # `prim_bind` / `run` rather than inspecting the struct.
      #
      # Structural equality is deliberately **not** defined:
      # comparing two effect-monad values would conflate "same
      # description" with "same outcome", which spec 11 §`run`
      # warns against (Ruby effects may be non-deterministic).
      # Default `Object#==` (identity) is the correct fallback.
      class Action
        # The three admitted kinds. Kept as a frozen constant so
        # that a misuse (e.g. a generated-code bug constructing
        # an `Action` with a bogus kind) surfaces as a
        # `BoundaryError` at construction time rather than during
        # `run`.
        KINDS = %i[pure embed bind].freeze

        attr_reader :kind, :payload

        def initialize(kind, payload)
          unless KINDS.include?(kind)
            raise Errors::BoundaryError,
                  "unknown Ruby action kind: #{kind.inspect}"
          end
          @kind = kind
          @payload = payload
          freeze
        end

        # A terser `inspect` than the default; keeps sensitive
        # closure internals out of logs.
        def inspect
          "#<Sapphire::Runtime::Ruby::Action kind=#{@kind}>"
        end
      end

      # `primReturn`: lift a Sapphire-ready value into a `Ruby a`
      # action whose execution yields the value immediately.
      #
      # Per spec 11 §Class instances the `Applicative Ruby` /
      # `Monad Ruby` instances are defined on top of this
      # primitive; the codegen (I7c) will emit a `pure` that
      # routes to `prim_return`.
      def self.prim_return(value)
        Action.new(:pure, value)
      end

      # `primBind`: compose `action : Ruby a` with a continuation
      # `k : a -> Ruby b`, yielding `Ruby b`.
      #
      # The continuation is taken as a Ruby `Proc` via the block
      # argument (generated code may also pass it explicitly via
      # `&proc`). It must return a `Ruby b`-shaped `Action`; `run`
      # validates this and raises if a raw (non-action) value
      # leaks through.
      def self.prim_bind(action, &k)
        unless action.is_a?(Action)
          raise Errors::BoundaryError,
                "prim_bind expects an Action as its first argument, got #{action.inspect}"
        end
        if k.nil?
          raise Errors::BoundaryError,
                "prim_bind requires a continuation block"
        end
        Action.new(:bind, [action, k])
      end

      # `primEmbed`: wrap a zero-argument `Proc` carrying
      # Ruby-side code (typically the compiled form of a `:=`-
      # bound snippet per spec 10 §The embedding form) as a
      # `Ruby a` action.
      #
      # The block is invoked only when the enclosing action is
      # `run`, per spec 11's deferred-evaluation semantics. Its
      # return value is passed through `Marshal.from_ruby` so the
      # continuation sees a Sapphire-ready value (spec 11 items 2
      # and 3 of the execution model). Exceptions raised inside
      # the block surface via `run` as an `Err RubyError`, per
      # spec 10 §Exception model (`StandardError` scope, B-03-OQ5).
      def self.prim_embed(&body)
        if body.nil?
          raise Errors::BoundaryError,
                "prim_embed requires a block carrying the Ruby snippet"
        end
        Action.new(:embed, body)
      end

      # Drive an action to completion.
      #
      # Returns a two-element Array with shape
      #
      #     [:ok,  sapphire_value]
      #     [:err, ruby_error]
      #
      # The `[:ok, ·]` / `[:err, ·]` pair is the R4 boundary
      # convention for the `Result RubyError a` shape per spec 11
      # §`run`; marshalling into the tagged-hash `Result` ADT
      # (`{ tag: :Ok, values: [a] }` / `{ tag: :Err, values: [e] }`)
      # is R5's job when it wires `run` into the generated Ruby
      # module. The flat-tuple shape here is deliberately friendly
      # for pattern-matching from Ruby (`case run(action) in [:ok,
      # v]; ...; end`) while preserving the short-circuit
      # semantics of spec 11 §Execution model item 5.
      #
      # Per spec 10 §Exception model the rescue scope is
      # `StandardError` — system-level exceptions (`Interrupt`,
      # `SystemExit`, `NoMemoryError`, `SystemStackError`, …)
      # propagate past the boundary by design.
      def self.run(action)
        unless action.is_a?(Action)
          raise Errors::BoundaryError,
                "run expects an Action, got #{action.inspect}"
        end
        begin
          value = evaluate(action)
        rescue StandardError => e
          return [:err, RubyError.from_exception(e)]
        end
        [:ok, value]
      end

      # Internal: evaluate an action and return its Sapphire-side
      # final value. Exceptions propagate; `run` is the single
      # catch point.
      #
      # The evaluator is iterative on the `:bind` spine so that
      # deeply chained `prim_bind` does not exhaust Ruby's call
      # stack. `:pure` and `:embed` leaves are evaluated inline;
      # the continuation's result, which must itself be an
      # `Action`, replaces the current action so the next step
      # runs on the same loop.
      def self.evaluate(action)
        current = action
        loop do
          case current.kind
          when :pure
            return current.payload
          when :embed
            raw = current.payload.call
            return Marshal.from_ruby(raw)
          when :bind
            ma, k = current.payload
            a = evaluate(ma)
            next_action = k.call(a)
            unless next_action.is_a?(Action)
              raise Errors::BoundaryError,
                    "prim_bind continuation must return a Ruby Action, got #{next_action.inspect}"
            end
            current = next_action
          end
        end
      end
      private_class_method :evaluate
    end
  end
end
