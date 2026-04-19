# frozen_string_literal: true

module Sapphire
  module Runtime
    # Tagged-hash ADT helpers.
    #
    # Per docs/spec/10-ruby-interop.md §ADTs, a Sapphire ADT value
    # `K v1 ... vk` marshals to a Ruby hash
    # `{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }`. This module
    # is the helper surface that generated Ruby code uses to build
    # and inspect those hashes.
    #
    # Design rationale:
    #
    # - The canonical value representation is a frozen `Hash` with
    #   exactly the two keys `:tag` (a Symbol) and `:values` (a
    #   frozen `Array`). Spec 10 §Design notes explicitly rejects
    #   "class per constructor" in favour of tagged hashes: this
    #   keeps the marshalling contract simple and avoids making the
    #   Ruby side depend on generated class definitions.
    # - Frozen-ness gives Sapphire's pure-value semantics cheaply
    #   (`Hash#freeze` plus per-element freeze of the `values`
    #   array). Structural equality (`==`) and `hash` fall out of
    #   Ruby's built-in `Hash` semantics for free — two ADT values
    #   with the same tag and fields are `eql?` without any extra
    #   code.
    # - The optional `define` DSL exposes uppercase-named class
    #   methods (e.g. `Color.Red`, `Maybe.Just(x)`) that match the
    #   shape spec 10 §Generated Ruby module shape fixes for
    #   constructor factories. It builds on top of `ADT.make`.
    #
    # See also: docs/impl/11-runtime-adt-marshalling.md.
    module ADT
      # Build a frozen tagged-hash ADT value.
      #
      # `tag` is normalised to a `Symbol`. `values` is expected to
      # be an `Array`; it is duplicated, frozen, and stored under
      # `:values`. The resulting hash itself is frozen so that
      # generated code cannot mutate ADT values in place.
      def self.make(tag, values = [])
        unless tag.is_a?(Symbol) || tag.is_a?(String)
          raise Errors::BoundaryError,
                "ADT tag must be a Symbol or String, got #{tag.class}"
        end
        unless values.is_a?(Array)
          raise Errors::BoundaryError,
                "ADT values must be an Array, got #{values.class}"
        end
        { tag: tag.to_sym, values: values.dup.freeze }.freeze
      end

      # Pattern-match helper. Yields `(tag, values)` to the block
      # when `value` is a tagged-hash ADT; raises
      # `Errors::BoundaryError` otherwise.
      def self.match(value)
        unless tagged?(value)
          raise Errors::BoundaryError,
                "expected tagged ADT hash, got #{value.inspect}"
        end
        yield value[:tag], value[:values]
      end

      # True if `value` conforms to the tagged-hash ADT shape
      # (a `Hash` with exactly the keys `:tag` and `:values`,
      # where `:tag` is a `Symbol` and `:values` is an `Array`).
      def self.tagged?(value)
        return false unless value.is_a?(Hash)
        return false unless value.size == 2
        return false unless value.key?(:tag) && value.key?(:values)
        value[:tag].is_a?(Symbol) && value[:values].is_a?(Array)
      end

      # Extract the `:tag` of a tagged-hash ADT value. Raises
      # `Errors::BoundaryError` on non-ADT input.
      def self.tag(value)
        raise Errors::BoundaryError, "expected tagged ADT hash, got #{value.inspect}" unless tagged?(value)
        value[:tag]
      end

      # Extract the `:values` of a tagged-hash ADT value. Raises
      # `Errors::BoundaryError` on non-ADT input.
      def self.values(value)
        raise Errors::BoundaryError, "expected tagged ADT hash, got #{value.inspect}" unless tagged?(value)
        value[:values]
      end

      # Ergonomic DSL: install a constructor factory method on
      # `target_module`.
      #
      #   Sapphire::Runtime::ADT.define(Color, :Red)
      #   Sapphire::Runtime::ADT.define(Maybe, :Just, arity: 1)
      #
      # After the call, `Color.Red` returns `{ tag: :Red, values: [] }`
      # and `Maybe.Just(3)` returns `{ tag: :Just, values: [3] }`.
      # Both results are frozen per `ADT.make`.
      #
      # Constructor names are preserved literally (capitalised) as
      # spec 10 §Generated Ruby module shape requires; Ruby admits
      # uppercase method names, so no mangling is performed.
      def self.define(target_module, tag, arity: 0)
        unless target_module.is_a?(Module)
          raise Errors::BoundaryError,
                "define target must be a Module, got #{target_module.class}"
        end
        unless tag.is_a?(Symbol) || tag.is_a?(String)
          raise Errors::BoundaryError,
                "ADT tag must be a Symbol or String, got #{tag.class}"
        end
        unless arity.is_a?(Integer) && arity >= 0
          raise Errors::BoundaryError,
                "arity must be a non-negative Integer, got #{arity.inspect}"
        end

        sym_tag = tag.to_sym
        method_name = sym_tag

        if arity.zero?
          target_module.define_singleton_method(method_name) do
            Sapphire::Runtime::ADT.make(sym_tag, [])
          end
        else
          params = Array.new(arity) { |i| "v#{i}" }.join(", ")
          target_module.module_eval(<<~RUBY, __FILE__, __LINE__ + 1)
            def self.#{method_name}(#{params})
              ::Sapphire::Runtime::ADT.make(#{sym_tag.inspect}, [#{params}])
            end
          RUBY
        end

        sym_tag
      end

      # Bulk form of `define`. Accepts a mapping `{ Tag => arity }`
      # (or an array of `[tag, arity]` pairs) and installs a factory
      # method on `target_module` for each variant.
      def self.define_variants(target_module, variants)
        unless variants.respond_to?(:each_pair) || variants.respond_to?(:each)
          raise Errors::BoundaryError,
                "variants must be enumerable, got #{variants.class}"
        end
        pairs = variants.respond_to?(:each_pair) ? variants.each_pair : variants.each
        pairs.map do |tag, arity|
          define(target_module, tag, arity: arity || 0)
        end
      end
    end
  end
end
