# frozen_string_literal: true

module Sapphire
  module Runtime
    # Ruby-side exception hierarchy used by the runtime itself.
    #
    # Per docs/build/03-sapphire-runtime.md §Errors namespace, these
    # are raised when runtime helpers are called with inputs that the
    # generated-code / runtime calling convention should have
    # prevented. They are NOT Sapphire-side `RubyError` values.
    #
    # When such an error is raised inside a running `Ruby a` action,
    # the boundary catch in `Sapphire::Runtime::Ruby.run` repackages
    # it as a `RubyError` like any other exception. Outside a
    # `Ruby a` action, they propagate normally.
    module Errors
      # Root of all runtime errors.
      class Base < StandardError; end

      # Raised by `Marshal.to_ruby` / `to_sapphire` when the input
      # shape disagrees with the declared Sapphire type.
      #
      # Implementation lands in R3 (Track R).
      class MarshalError < Base; end

      # Raised by `ADT.match` (and similar helpers) when a
      # non-tagged value reaches a point that requires one.
      #
      # Implementation lands in R2 (Track R).
      class BoundaryError < Base; end
    end
  end
end
