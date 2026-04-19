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
      # The class itself is defined here in R1; raise sites land in
      # R3 when the Marshal helpers are implemented.
      class MarshalError < Base; end

      # Raised by `ADT.match` (and similar helpers) when a
      # non-tagged value reaches a point that requires one.
      #
      # The class itself is defined here in R1; raise sites land in
      # R2 when the ADT helpers are implemented.
      class BoundaryError < Base; end

      # Raised by `Sapphire::Runtime.require_version!` when the
      # caller-supplied version constraint is not satisfied by the
      # loaded runtime gem (`Sapphire::Runtime::VERSION`).
      #
      # Per docs/impl/16-runtime-threaded-loading.md §R6 loading
      # 契約 and docs/build/03-sapphire-runtime.md §Versioning and
      # the calling convention, this is the error generated code
      # sees when the runtime gem it was compiled against is not
      # compatible with the runtime gem actually loaded at Ruby
      # execution time. The message names both versions so the
      # user can reconcile by pinning `sapphire-runtime` in their
      # `Gemfile`.
      class RuntimeVersionMismatch < Base; end

      # Raised when the runtime cannot satisfy a `require_version!`
      # call because its argument is malformed (not a Gem-style
      # requirement string). This is distinct from
      # `RuntimeVersionMismatch`, which is a satisfiable-but-failed
      # constraint; `LoadError` signals that the constraint itself
      # could not be parsed.
      #
      # Named `LoadError` (not Ruby's top-level `LoadError`) to
      # stay inside the `Sapphire::Runtime::Errors` hierarchy: it
      # is a `StandardError` like the others in this module, so
      # `Ruby.run`'s boundary rescue will repackage it as a
      # `RubyError` if raised inside a running action, rather than
      # propagating past the boundary the way Ruby's own
      # `LoadError` would.
      class LoadError < Base; end
    end
  end
end
