# frozen_string_literal: true

module Sapphire
  module Runtime
    # The `Ruby` monad evaluator.
    #
    # Per docs/spec/11-ruby-monad.md and
    # docs/build/03-sapphire-runtime.md §The `Ruby` monad evaluator,
    # this module exposes the three primitives the generated code
    # invokes:
    #
    # - `pure(value)`   — produces a `Ruby a` action whose execution
    #                     yields `value` immediately (primReturn).
    # - `bind(action, k)` — sequentially composes a `Ruby a` with a
    #                     Sapphire continuation (primBind).
    # - `run(action)`   — drives an action to completion, returning
    #                     a `Result RubyError a`-shaped Sapphire
    #                     value on a fresh Ruby thread.
    #
    # There is deliberately no `unsafeRun` / escape hatch (spec 11
    # §There is no `unsafeRun` / `runIO`).
    #
    # TODO: implement in R4 (see docs/impl/06-implementation-roadmap.md
    # §Track R).
    module Ruby
    end
  end
end
