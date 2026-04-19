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

require "sapphire/runtime/version"
require "sapphire/runtime/errors"
require "sapphire/runtime/adt"
require "sapphire/runtime/marshal"
require "sapphire/runtime/ruby_error"
require "sapphire/runtime/ruby"

module Sapphire
  module Runtime
  end
end
