# frozen_string_literal: true

# R6 — `Sapphire::Runtime.require_version!` spec.
#
# Per docs/impl/16-runtime-threaded-loading.md §R6 loading 契約
# and docs/build/03-sapphire-runtime.md §Versioning and the
# calling convention, generated code asserts version
# compatibility with the loaded `sapphire-runtime` gem through
# `Sapphire::Runtime.require_version!(constraint)`. Satisfiable
# constraints return the loaded version; malformed or
# unsatisfied constraints raise inside the
# `Sapphire::Runtime::Errors` hierarchy.

RSpec.describe Sapphire::Runtime, ".require_version! (R6 loading contract)" do
  let(:runtime) { described_class }
  let(:loaded_version) { Sapphire::Runtime::VERSION }

  describe "satisfied constraints" do
    it "returns the loaded version on exact match" do
      expect(runtime.require_version!(loaded_version)).to eq(loaded_version)
    end

    it "accepts a pessimistic (`~>`) constraint that covers the loaded version" do
      # For 0.1.0 the constraint `~> 0.1` admits 0.1.x and any
      # later 0.y.z < 1.0.
      expect(runtime.require_version!("~> 0.1")).to eq(loaded_version)
    end

    it "accepts a tighter `~> 0.1.0` that still covers 0.1.x" do
      expect(runtime.require_version!("~> 0.1.0")).to eq(loaded_version)
    end

    it "accepts an Array of individually-satisfied constraints" do
      expect(runtime.require_version!([">= 0.1.0", "< 1.0"])).to eq(loaded_version)
    end

    it "accepts `>= 0` as a trivially-satisfied constraint" do
      expect(runtime.require_version!(">= 0")).to eq(loaded_version)
    end
  end

  describe "unsatisfied constraints" do
    it "raises RuntimeVersionMismatch on a future major (`~> 99.0`)" do
      expect { runtime.require_version!("~> 99.0") }
        .to raise_error(Sapphire::Runtime::Errors::RuntimeVersionMismatch)
    end

    it "raises RuntimeVersionMismatch on a strict greater-than loaded" do
      expect { runtime.require_version!("> #{loaded_version}") }
        .to raise_error(Sapphire::Runtime::Errors::RuntimeVersionMismatch)
    end

    it "error message names both the constraint and the loaded version" do
      runtime.require_version!("~> 99.0")
    rescue Sapphire::Runtime::Errors::RuntimeVersionMismatch => e
      expect(e.message).to include("99")
      expect(e.message).to include(loaded_version)
    end

    it "error message suggests a Gemfile pin" do
      runtime.require_version!("~> 99.0")
    rescue Sapphire::Runtime::Errors::RuntimeVersionMismatch => e
      expect(e.message).to include("Gemfile")
    end

    it "rejects a compound constraint whose upper bound excludes the loaded version" do
      expect { runtime.require_version!([">= 0.0.1", "< 0.1.0"]) }
        .to raise_error(Sapphire::Runtime::Errors::RuntimeVersionMismatch)
    end
  end

  describe "malformed constraints" do
    it "raises LoadError on nil" do
      expect { runtime.require_version!(nil) }
        .to raise_error(Sapphire::Runtime::Errors::LoadError)
    end

    it "raises LoadError on an unparseable string" do
      expect { runtime.require_version!("not a version") }
        .to raise_error(Sapphire::Runtime::Errors::LoadError)
    end

    it "raises LoadError on a non-String / non-Array argument" do
      expect { runtime.require_version!(Object.new) }
        .to raise_error(Sapphire::Runtime::Errors::LoadError)
    end

    it "names the offending type by class name in the error message" do
      # The message should read "got Object" rather than the
      # default `inspect` output (`#<Object:0x...>`), which is
      # unhelpful to a user who just needs to know they passed
      # the wrong type.
      runtime.require_version!(Object.new)
    rescue Sapphire::Runtime::Errors::LoadError => e
      expect(e.message).to include("got Object")
      expect(e.message).not_to include("#<Object")
    end

    it "falls back to 'anonymous class' for Class.new singletons" do
      anon_instance = Class.new.new
      runtime.require_version!(anon_instance)
    rescue Sapphire::Runtime::Errors::LoadError => e
      expect(e.message).to include("anonymous class")
    end

    it "falls back to inspect for nil (class name 'NilClass' is still usable though)" do
      # nil's class is NilClass (name = "NilClass"), so the
      # diagnostic reads "got NilClass" — the inspect-fallback
      # branch is only reached for exotic BasicObject subclasses
      # that override `class`.
      runtime.require_version!(nil)
    rescue Sapphire::Runtime::Errors::LoadError => e
      expect(e.message).to include("NilClass")
    end

    it "LoadError is distinct from Ruby's top-level LoadError" do
      # The runtime's namespaced LoadError is a Sapphire-runtime
      # StandardError, not Ruby's ::LoadError which is a
      # ScriptError. This matters for `Ruby.run`'s rescue scope
      # (it covers StandardError only).
      expect(Sapphire::Runtime::Errors::LoadError.ancestors)
        .to include(Sapphire::Runtime::Errors::Base)
      expect(Sapphire::Runtime::Errors::LoadError.ancestors).to include(StandardError)
      expect(Sapphire::Runtime::Errors::LoadError).not_to be < ::LoadError
    end
  end

  describe "error hierarchy" do
    it "RuntimeVersionMismatch is a Sapphire::Runtime::Errors::Base" do
      expect(Sapphire::Runtime::Errors::RuntimeVersionMismatch.ancestors)
        .to include(Sapphire::Runtime::Errors::Base)
    end

    it "RuntimeVersionMismatch is a StandardError (so Ruby.run repackages it)" do
      expect(Sapphire::Runtime::Errors::RuntimeVersionMismatch.ancestors)
        .to include(StandardError)
    end

    it "when raised inside a Ruby action, Ruby.run wraps it as [:err, RubyError]" do
      # Contract per docs/build/03-sapphire-runtime.md §Errors
      # namespace: runtime errors raised inside a running action
      # are repackaged as RubyError like any other StandardError.
      action = Sapphire::Runtime::Ruby.prim_embed do
        Sapphire::Runtime.require_version!("~> 99.0")
      end
      status, err = Sapphire::Runtime::Ruby.run(action)

      expect(status).to eq(:err)
      class_name, message, _bt = Sapphire::Runtime::ADT.values(err)
      expect(class_name).to eq("Sapphire::Runtime::Errors::RuntimeVersionMismatch")
      expect(message).to include(loaded_version)
    end
  end

  describe "loaded public surface" do
    # R6 contract item: `require "sapphire/runtime"` loads the
    # full public surface in one call.
    it "defines Sapphire::Runtime::ADT" do
      expect(defined?(Sapphire::Runtime::ADT)).to eq("constant")
    end

    it "defines Sapphire::Runtime::Marshal" do
      expect(defined?(Sapphire::Runtime::Marshal)).to eq("constant")
    end

    it "defines Sapphire::Runtime::Ruby" do
      expect(defined?(Sapphire::Runtime::Ruby)).to eq("constant")
    end

    it "defines Sapphire::Runtime::RubyError" do
      expect(defined?(Sapphire::Runtime::RubyError)).to eq("constant")
    end

    it "defines Sapphire::Runtime::Errors and its new R6 subclasses" do
      expect(defined?(Sapphire::Runtime::Errors)).to eq("constant")
      expect(defined?(Sapphire::Runtime::Errors::RuntimeVersionMismatch)).to eq("constant")
      expect(defined?(Sapphire::Runtime::Errors::LoadError)).to eq("constant")
    end

    it "defines Sapphire::Runtime::VERSION as a non-empty String" do
      expect(Sapphire::Runtime::VERSION).to be_a(String)
      expect(Sapphire::Runtime::VERSION).not_to be_empty
    end
  end
end
