# frozen_string_literal: true

module Sapphire
  module Runtime
    # Helpers that build the Sapphire-side `RubyError` tagged-hash
    # value from a caught Ruby `Exception`.
    #
    # Per docs/spec/10-ruby-interop.md §Exception model, the
    # Sapphire-side type is:
    #
    #     data RubyError = RubyError String String (List String)
    #                                class_name   message  backtrace
    #
    # The runtime catches `StandardError` (and subclasses) at the
    # boundary inside `Sapphire::Runtime::Ruby.run` and converts
    # them through this module. System-level exceptions
    # (`Interrupt`, `SystemExit`, `NoMemoryError`,
    # `SystemStackError`, etc.) propagate past the boundary by
    # design (B-03-OQ5, closed 2026-04-18).
    #
    # TODO: implement in R5 (see docs/impl/06-implementation-roadmap.md
    # §Track R).
    module RubyError
    end
  end
end
