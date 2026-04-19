# frozen_string_literal: true

module Sapphire
  module Runtime
    # Type-directed marshalling between Sapphire values and Ruby
    # values across the boundary.
    #
    # Per docs/spec/10-ruby-interop.md §Data model and
    # docs/build/03-sapphire-runtime.md §Marshalling helpers, this
    # module exposes two helpers:
    #
    # - `to_ruby(sapphire_value, type)` — Sapphire -> Ruby, used
    #   when generated code hands a Sapphire value to a
    #   `:=`-bound Ruby snippet.
    # - `to_sapphire(ruby_value, type)` — Ruby -> Sapphire, used
    #   when a Ruby snippet's result re-enters Sapphire. Raises
    #   `Sapphire::Runtime::Errors::MarshalError` on a shape
    #   mismatch.
    #
    # Both are type-directed: the type argument is the authoritative
    # oracle for which marshalling rule to apply.
    #
    # TODO: implement in R3 (see docs/impl/06-implementation-roadmap.md
    # §Track R).
    module Marshal
    end
  end
end
