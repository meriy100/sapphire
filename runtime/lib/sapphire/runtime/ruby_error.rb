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
    # ## R4 scope
    #
    # This R4 change implements the minimal `from_exception`
    # constructor that `Ruby.run` needs to produce `[:err, e]`
    # tuples. R5 (docs/impl/06-implementation-roadmap.md §Track R)
    # will expand this module if it needs additional surface
    # (e.g. when wiring the tuple-shaped result into the final
    # `Result RubyError a` tagged-hash shape).
    module RubyError
      # Build a Sapphire-side `RubyError` ADT value from a caught
      # `Exception`.
      #
      # Marshals to `{ tag: :RubyError, values: [class_name,
      # message, backtrace] }` per spec 10 §Exception model, with:
      #
      # - `class_name` — `e.class.name`, falling back to the empty
      #   string for anonymous classes.
      # - `message`    — `e.message.to_s`, to tolerate exceptions
      #   whose `#message` returns a non-String (rare but admitted
      #   by Ruby).
      # - `backtrace`  — `e.backtrace || []`. Ruby occasionally
      #   leaves `backtrace` as `nil` for exceptions that were
      #   never raised (e.g. constructed via `.new` and handed
      #   around); spec 10 §Exception model permits substituting
      #   an empty list in that case.
      #
      # The returned hash is frozen per `ADT.make`; every payload
      # string is forced to UTF-8 and frozen, matching the
      # boundary contract for `String` values in spec 10 §Ground
      # types.
      def self.from_exception(e)
        class_name = sanitize_string(e.class.name || "")
        message    = sanitize_string(e.message.to_s)
        backtrace  = (e.backtrace || []).map { |line| sanitize_string(line.to_s) }.freeze
        ADT.make(:RubyError, [class_name, message, backtrace])
      end

      # Coerce a Ruby string to valid UTF-8 and freeze it. Invalid
      # byte sequences are replaced rather than raising: the
      # boundary is carrying a best-effort description of a
      # failure, and producing a second failure from a bad
      # backtrace line would obscure the original error.
      def self.sanitize_string(s)
        str = s.to_s
        if str.encoding == Encoding::UTF_8
          str = str.scrub("?") unless str.valid_encoding?
        else
          str = str.encode(
            Encoding::UTF_8,
            invalid: :replace,
            undef:   :replace,
            replace: "?",
          )
        end
        str.frozen? ? str : str.freeze
      end
      private_class_method :sanitize_string
    end
  end
end
