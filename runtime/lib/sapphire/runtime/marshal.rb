# frozen_string_literal: true

module Sapphire
  module Runtime
    # Boundary marshalling between Sapphire values and Ruby values.
    #
    # This R3 surface is the *shape-driven* subset of the full
    # type-directed contract sketched in
    # docs/build/03-sapphire-runtime.md §Marshalling helpers. The
    # type-argument form (`to_ruby(value, type)` /
    # `to_sapphire(value, type)`) is still expected for R4..R6;
    # the helpers below cover the cases where Sapphire's and
    # Ruby's representations are already congruent enough that the
    # runtime can route by structure alone:
    #
    # - `Int`    <-> `Integer`       (spec 10 §Ground types)
    # - `String` <-> `String` UTF-8  (spec 10 §Ground types)
    # - `Bool`   <-> `true` / `false` (spec 10 §Ground types)
    # - `List a` <-> `Array`          (spec 10 §Lists)
    # - record   <-> symbol-keyed `Hash` (spec 10 §Records, 10-OQ3)
    # - ADT      <-> tagged `Hash`    (spec 10 §ADTs)
    #
    # Deliberately excluded (each raises `MarshalError`):
    #
    # - `Float` — Sapphire has no floating point until 07-OQ6
    #   lands. Ruby-side `Float`s entering the boundary are a
    #   shape error, not a silent coercion.
    # - `nil` — `nil` has no Sapphire counterpart; 10-OQ1 closed
    #   the `nil <-> Nothing` shortcut as not admitted.
    # - `Symbol` other than `:lt` / `:eq` / `:gt` — these three are
    #   the marshalled form of `Ordering` (spec 10 §Ordering).
    #   Arbitrary Ruby symbols do not cross the boundary.
    # - Arbitrary Ruby objects. Opaque values produced by future
    #   `ruby_eval`-style escape hatches are out of scope for R3.
    #
    # The full type-directed variant (`to_ruby(value, type)`) will
    # fold the R3 shape-driven core as a default cascade and add
    # type-level discrimination for records-vs-ADTs (which share
    # hash representations; spec 10 notes this disambiguation is
    # expected-type-driven). See docs/impl/11-runtime-adt-
    # marshalling.md for the transition plan.
    module Marshal
      # Three-element intern set for `Ordering`.
      ORDERING_SYMBOLS = %i[lt eq gt].freeze

      # Take a Ruby-side value at the boundary (for example the
      # payload of a `:=`-bound snippet's result) and produce a
      # Sapphire-ready value, recursively. Raises
      # `Errors::MarshalError` for shapes the shape-driven subset
      # does not recognise.
      def self.from_ruby(value)
        case value
        when true, false
          value
        when Integer
          # Ruby's `Integer` includes arbitrary-precision integers;
          # Sapphire `Int` is arbitrary-precision per spec 05, so
          # this is total.
          value
        when Float
          raise Errors::MarshalError,
                "Float is not supported at the boundary (see 07-OQ6); got #{value.inspect}"
        when String
          # Freeze and ensure UTF-8 encoding per spec 10 §Ground types.
          s = value.encoding == Encoding::UTF_8 ? value : value.dup.force_encoding(Encoding::UTF_8)
          unless s.valid_encoding?
            raise Errors::MarshalError,
                  "String is not valid UTF-8: #{value.inspect}"
          end
          s.frozen? ? s : s.freeze
        when Symbol
          if ORDERING_SYMBOLS.include?(value)
            value
          else
            raise Errors::MarshalError,
                  "Symbol #{value.inspect} is not a valid boundary value (only :lt/:eq/:gt for Ordering)"
          end
        when Array
          value.map { |e| from_ruby(e) }.freeze
        when Hash
          if ADT.tagged?(value)
            ADT.make(value[:tag], value[:values].map { |e| from_ruby(e) })
          elsif symbol_keyed?(value)
            value.each_with_object({}) { |(k, v), acc| acc[k] = from_ruby(v) }.freeze
          else
            raise Errors::MarshalError,
                  "Hash must either be a tagged ADT (keys :tag/:values) or symbol-keyed record; got #{value.inspect}"
          end
        when nil
          raise Errors::MarshalError,
                "nil is not a valid boundary value (see 10-OQ1)"
        else
          raise Errors::MarshalError,
                "unsupported Ruby value at boundary: #{value.class} #{value.inspect}"
        end
      end

      # Produce a Ruby-side value from a Sapphire-ready value
      # (already frozen, already in the canonical representation).
      #
      # For R3, the shape-driven subset is *idempotent* on
      # well-formed Sapphire-side values: Sapphire `Int` already is
      # a Ruby `Integer`, etc. The helper still validates the shape
      # so that a misuse by generated code surfaces as
      # `Errors::MarshalError` rather than silently propagating.
      def self.to_ruby(value)
        case value
        when true, false, Integer
          value
        when Float
          raise Errors::MarshalError,
                "Float is not supported at the boundary (see 07-OQ6); got #{value.inspect}"
        when String
          value.frozen? ? value : value.dup.freeze
        when Symbol
          unless ORDERING_SYMBOLS.include?(value)
            raise Errors::MarshalError,
                  "Symbol #{value.inspect} is not a valid Sapphire value outside Ordering"
          end
          value
        when Array
          value.map { |e| to_ruby(e) }
        when Hash
          if ADT.tagged?(value)
            ADT.make(value[:tag], value[:values].map { |e| to_ruby(e) })
          elsif symbol_keyed?(value)
            value.each_with_object({}) { |(k, v), acc| acc[k] = to_ruby(v) }
          else
            raise Errors::MarshalError,
                  "Hash must either be a tagged ADT or symbol-keyed record; got #{value.inspect}"
          end
        when nil
          raise Errors::MarshalError,
                "nil is not a valid Sapphire value (see 10-OQ1)"
        else
          raise Errors::MarshalError,
                "unsupported Sapphire-side value: #{value.class} #{value.inspect}"
        end
      end

      # True if every key of `hash` is a Symbol. Empty hashes are
      # admitted (they represent the empty record `{}` per spec
      # 04).
      def self.symbol_keyed?(hash)
        hash.is_a?(Hash) && hash.keys.all?(Symbol)
      end
    end
  end
end
