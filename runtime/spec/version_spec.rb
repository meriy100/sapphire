# frozen_string_literal: true

RSpec.describe Sapphire::Runtime do
  describe "VERSION" do
    it "is a non-empty string" do
      expect(Sapphire::Runtime::VERSION).to be_a(String)
      expect(Sapphire::Runtime::VERSION).not_to be_empty
    end

    it "is 0.1.0 at the R1 scaffold" do
      # This is the initial scaffold version per
      # docs/impl/08-runtime-layout.md §Gem identity. R2..R6 must
      # not bump this without revisiting that document.
      expect(Sapphire::Runtime::VERSION).to eq("0.1.0")
    end
  end

  describe "public surface" do
    it "exposes the sub-modules declared in the build 03 contract" do
      # docs/build/03-sapphire-runtime.md §Sub-module map fixes
      # these five names; the R1 scaffold wires them as empty
      # modules so generated code can depend on the names.
      expect(Sapphire::Runtime.const_defined?(:ADT)).to be(true)
      expect(Sapphire::Runtime.const_defined?(:Marshal)).to be(true)
      expect(Sapphire::Runtime.const_defined?(:Ruby)).to be(true)
      expect(Sapphire::Runtime.const_defined?(:RubyError)).to be(true)
      expect(Sapphire::Runtime.const_defined?(:Errors)).to be(true)
    end

    it "defines the three errors subclasses used by later tracks" do
      expect(Sapphire::Runtime::Errors::Base.ancestors).to include(StandardError)
      expect(Sapphire::Runtime::Errors::MarshalError.ancestors)
        .to include(Sapphire::Runtime::Errors::Base)
      expect(Sapphire::Runtime::Errors::BoundaryError.ancestors)
        .to include(Sapphire::Runtime::Errors::Base)
    end
  end
end
