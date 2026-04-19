# frozen_string_literal: true

# Entry point for the sapphire-runtime gem.
#
# Per docs/build/03-sapphire-runtime.md §Loading and `require`
# order, generated Sapphire modules start with `require
# "sapphire/runtime"`. That single require loads the full
# public surface (ADT / Marshal / Ruby / RubyError / Errors).
#
# Sub-path requires (e.g. `require "sapphire/runtime/adt"`) are
# admitted but not required by the contract.
#
# The `rubygems` require is intentional and up-front: R6 exposes
# `Sapphire::Runtime.require_version!`, whose implementation uses
# `Gem::Requirement` / `Gem::Version`. Ruby makes `Gem::*`
# available whenever the gem was loaded via `require
# "sapphire/runtime"` under Bundler / `gem install`, but a bare
# `ruby -I runtime/lib ...` invocation does not pre-require
# `rubygems` by default, so the helper guards itself with this
# explicit require.

require "rubygems"
require "sapphire/runtime/version"
require "sapphire/runtime/errors"
require "sapphire/runtime/adt"
require "sapphire/runtime/marshal"
require "sapphire/runtime/ruby_error"
require "sapphire/runtime/ruby"

module Sapphire
  module Runtime
    # Assert that the loaded runtime gem satisfies the given
    # Gem-style version constraint.
    #
    # Generated code (I7c) emits a call to this helper near the
    # top of every Sapphire module so that a runtime mismatch
    # between compile-time and load-time surfaces as a precise
    # error, rather than as a mysterious `NoMethodError` on a
    # primitive that moved or a silent behavioural change.
    #
    # The contract (docs/impl/16-runtime-threaded-loading.md §R6
    # loading 契約):
    #
    # 1. `constraint` is a `String` or `Array[String]` in the
    #    format `Gem::Requirement` accepts (e.g. `"~> 0.1"`,
    #    `[">= 0.1.0", "< 0.2"]`). Anything else raises
    #    `Errors::LoadError`.
    # 2. If the constraint parses but the loaded
    #    `Sapphire::Runtime::VERSION` does not satisfy it, this
    #    raises `Errors::RuntimeVersionMismatch` with a message
    #    that names both the required constraint and the loaded
    #    version.
    # 3. On success (constraint parses and is satisfied) the
    #    method returns the loaded version string so callers can
    #    log it if they want.
    #
    # The helper is deliberately callable from plain Ruby (outside
    # a `Ruby a` action), because the generated per-module file
    # invokes it at load time, before any action is constructed.
    # If it were ever called from inside a running action, the
    # boundary rescue in `Sapphire::Runtime::Ruby.run` would
    # repackage any `StandardError` raise as a `RubyError` like
    # any other runtime error.
    def self.require_version!(constraint)
      # Reject argument shapes `Gem::Requirement.create` would
      # silently normalise to `>= 0`. Without this guard, callers
      # that pass a wrong-typed value (e.g. a stray
      # `Object.new`) would see their version check spuriously
      # succeed, which defeats the whole point of the helper.
      unless constraint.is_a?(String) ||
             constraint.is_a?(Array) ||
             constraint.is_a?(Gem::Requirement) ||
             constraint.is_a?(Gem::Version)
        raise Errors::LoadError,
              "require_version! expects a String, Array[String], " \
              "Gem::Requirement, or Gem::Version constraint; got " \
              "#{constraint.class.name || constraint.inspect}"
      end

      if constraint.is_a?(Array) && constraint.any? { |c| !c.is_a?(String) }
        raise Errors::LoadError,
              "require_version! expects every element of an Array " \
              "constraint to be a String; got #{constraint.inspect}"
      end

      requirement =
        begin
          Gem::Requirement.create(constraint)
        rescue ArgumentError, TypeError => e
          raise Errors::LoadError,
                "invalid version constraint for sapphire-runtime: " \
                "#{constraint.inspect} (#{e.message})"
        end

      loaded = Gem::Version.new(VERSION)
      return VERSION if requirement.satisfied_by?(loaded)

      raise Errors::RuntimeVersionMismatch,
            "sapphire-runtime version mismatch: generated code " \
            "requires #{requirement}, but Sapphire::Runtime::VERSION " \
            "is #{VERSION}. Pin `gem \"sapphire-runtime\", " \
            "#{requirement.to_s.inspect}` in your Gemfile or " \
            "re-run the Sapphire compiler against the installed " \
            "runtime."
    end
  end
end
