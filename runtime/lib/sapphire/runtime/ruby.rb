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
    # completes before the next begins. R5 (this implementation)
    # additionally honours spec 11 item 1's "fresh Ruby evaluator
    # thread per `run`" contract by spawning a dedicated
    # `Thread.new { ... }.value` per `run` invocation. The
    # observable semantics here satisfy spec 11 §Execution model
    # items 1-5:
    #
    # - item 1 (blocking caller, fresh thread): `run` joins the
    #   evaluator thread via `Thread#value` before returning.
    # - item 2 (sub-steps on that thread): every `:pure` / `:embed`
    #   / `:bind` step runs inside the spawned thread's body.
    # - item 3 (sequential `>>=`): the evaluator loop is
    #   synchronous; no scheduling is admitted.
    # - item 4 (per-step fresh local scope): each `prim_embed`
    #   block is a fresh Ruby closure with fresh block-locals.
    # - item 5 (raise short-circuits): the first raised
    #   `StandardError` propagates out of the evaluator loop.
    #
    # ### What is and is not isolated across `run` calls
    #
    # Fully isolated (docs/impl/16-runtime-threaded-loading.md
    # §分離の境界):
    #
    # - Ruby local variables of the snippet block (fresh block
    #   scope per embed, plus a fresh `Thread` so no caller-side
    #   locals leak in).
    # - `Thread.current[:...]` fibre-local / thread-local
    #   storage — a fresh `Thread` means a fresh storage scope,
    #   per spec 11 §Execution model item 4.
    #
    # Shared (with the caller thread and other `run`s) by design,
    # because the Ruby VM offers no in-process isolation for them
    # without switching to `fork` (CoW, not portable) or Ractor
    # (would also isolate immutable constants, which generated
    # code depends on):
    #
    # - Global variables (`$...`), top-level constants, and the
    #   `require` / `$LOADED_FEATURES` table.
    # - Monkey-patches to core classes, class variables, and any
    #   other process-wide mutable state.
    #
    # Generated code (I7c) therefore must not rely on
    # `run`-to-`run` isolation of Ruby global state; the spec-11
    # "fresh Ruby-side scope" is interpreted per
    # `docs/impl/16-runtime-threaded-loading.md` to cover locals
    # and thread-locals only.
    #
    # ### Reentrant `run`
    #
    # Reentrant invocations (`Ruby.run(inner)` called from inside
    # a `prim_embed` block whose outer `run` is still on the
    # stack) are admitted (I-OQ47). Each reentrant call spawns
    # its own evaluator thread and joins it before returning,
    # yielding independent `[:ok, _]` / `[:err, _]` results with
    # no state shared between inner and outer evaluator threads
    # (beyond the unavoidable process-global state listed above).
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

        # `kind` / `payload` are the internal discriminator and its
        # associated closure / value. They are **private** and
        # exposed to the evaluator (`Sapphire::Runtime::Ruby.run` /
        # `.evaluate`) inside this same module only. External Ruby
        # code, including user code and generated code, must treat
        # `Action` as opaque per spec 11 §Type signature, i.e.
        # interact exclusively via `prim_return` / `prim_embed` /
        # `prim_bind` / `run`. The readers are declared here purely
        # for debugging / introspection helpers within the runtime
        # itself (and for `inspect` below); they are not a public
        # surface.
        attr_reader :kind, :payload
        private :kind, :payload

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
        #
        # The `kind=` hint it emits is debug-only. Code must not
        # parse it to branch behaviour — treat `Action` as opaque
        # (spec 11 §Type signature) and go through the `prim_*`
        # primitives + `run` instead.
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
      # The `[:ok, ·]` / `[:err, ·]` pair is the R4/R5 boundary
      # convention for the `Result RubyError a` shape per spec 11
      # §`run`; marshalling into the tagged-hash `Result` ADT
      # (`{ tag: :Ok, values: [a] }` / `{ tag: :Err, values: [e] }`)
      # is deferred to the generated code layer (I7c) per I-OQ40
      # (DEFERRED-IMPL, status unchanged after R5). The flat-tuple
      # shape is deliberately friendly for pattern-matching from
      # Ruby (`case run(action) in [:ok, v]; ...; end`) while
      # preserving the short-circuit semantics of spec 11
      # §Execution model item 5.
      #
      # Per spec 10 §Exception model the rescue scope is
      # `StandardError` — system-level exceptions (`Interrupt`,
      # `SystemExit`, `NoMemoryError`, `SystemStackError`, …)
      # propagate past the boundary by design. When they are raised
      # inside the evaluator thread, `Thread#value` re-raises them
      # on the caller thread so the propagation still crosses the
      # `run` boundary intact.
      def self.run(action)
        unless action.is_a?(Action)
          raise Errors::BoundaryError,
                "run expects an Action, got #{action.inspect}"
        end
        # Spawn a fresh evaluator thread per `run` invocation (spec
        # 11 §Execution model item 1). `Thread#value` blocks the
        # caller until the evaluator completes and re-raises any
        # exception that escaped the evaluator thread, which is
        # how `Interrupt` / `SystemExit` / other non-StandardError
        # signals cross the boundary unaltered (B-03-OQ5 DECIDED).
        #
        # The thread body disables Ruby's default "thread
        # terminated with exception" stderr trail (`Thread.current
        # .report_on_exception = false`, in-body so it applies
        # before any work runs in the new thread). The exception is
        # still captured by `Thread#value` and re-raised on the
        # caller thread, which is the correct place to attribute
        # the trail if the caller does not rescue it. The R4-style
        # `[:ok, _]` / `[:err, _]` conversion also happens inside
        # the thread body, so the tuple itself is the thread's
        # return value in the success / StandardError cases.
        thread = Thread.new do
          # Set inside the new thread (rather than on the returned
          # Thread object from the caller side) so the flag is
          # already in place when the body begins executing —
          # there is no window in which an early raise could trip
          # MRI's default stderr trail. The exception is still
          # captured by Thread#value below and re-raised on the
          # caller thread, which is where the trail should be
          # attributed if the caller does not rescue it.
          Thread.current.report_on_exception = false
          begin
            value = evaluate(action)
            [:ok, value]
          rescue StandardError => e
            [:err, RubyError.from_exception(e)]
          end
        end
        thread.value
      end

      # Internal: evaluate an action and return its Sapphire-side
      # final value. Exceptions propagate; `run` is the single
      # catch point.
      #
      # ## Bind-spine iterativity (design note)
      #
      # The evaluator is iterative on the **right spine** of
      # `:bind`: once an upstream action evaluates to a value, the
      # continuation's resulting `Action` replaces `current` and
      # the `loop` consumes it without growing the Ruby call stack.
      # Sapphire's do-notation desugar emits **right-associated**
      # bind chains (`m >>= \x -> (k1 x >>= \y -> k2 y >>= ...)`),
      # so every chain that arrives from Sapphire source is safely
      # handled by this loop regardless of depth.
      #
      # **Hand-crafted left-associated bind chains**
      # (`((m >>= f) >>= g) >>= h`, equivalent to `foldl (>>=)` in
      # Haskell) do *not* enjoy the same guarantee: the left
      # argument is evaluated by a recursive `evaluate(ma)` call,
      # so N levels of left-associated `prim_bind` nest N frames on
      # the Ruby call stack. M9-range code does not produce such
      # chains (they do not arise from Sapphire's do-notation), but
      # a Ruby-side caller that builds them explicitly — notably a
      # `foldl (>>=)` pattern — can hit `SystemStackError` for
      # sufficiently deep chains. If that ever matters in practice
      # we would rebalance to the right at `prim_bind` construction
      # (or rework `evaluate` to an explicit work-stack); right-
      # associated chains stay tail-consumed in either design.
      def self.evaluate(action)
        current = action
        loop do
          case current.send(:kind)
          when :pure
            return current.send(:payload)
          when :embed
            raw = current.send(:payload).call
            return Marshal.from_ruby(raw)
          when :bind
            ma, k = current.send(:payload)
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
