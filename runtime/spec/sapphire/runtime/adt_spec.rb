# frozen_string_literal: true

RSpec.describe Sapphire::Runtime::ADT do
  let(:adt) { described_class }

  describe ".make" do
    it "builds a frozen tagged hash from a symbol tag" do
      v = adt.make(:Just, [42])
      expect(v).to eq({ tag: :Just, values: [42] })
      expect(v).to be_frozen
      expect(v[:values]).to be_frozen
    end

    it "normalises a string tag to a symbol" do
      v = adt.make("Nothing", [])
      expect(v[:tag]).to eq(:Nothing)
    end

    it "defaults values to an empty frozen array" do
      v = adt.make(:Nothing)
      expect(v[:values]).to eq([])
      expect(v[:values]).to be_frozen
    end

    it "copies the incoming values array (caller mutation is isolated)" do
      src = [1, 2]
      v = adt.make(:Pair, src)
      src << 3
      expect(v[:values]).to eq([1, 2])
    end

    it "rejects a non-symbol, non-string tag" do
      expect { adt.make(42, []) }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end

    it "rejects a non-array values payload" do
      expect { adt.make(:K, "oops") }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end
  end

  describe "structural equality" do
    it "treats two ADT values with the same tag and fields as equal" do
      a = adt.make(:Just, [1])
      b = adt.make(:Just, [1])
      expect(a).to eq(b)
      expect(a.hash).to eq(b.hash)
    end

    it "distinguishes different tags" do
      expect(adt.make(:Just, [1])).not_to eq(adt.make(:Nothing, []))
    end

    it "distinguishes different fields" do
      expect(adt.make(:Just, [1])).not_to eq(adt.make(:Just, [2]))
    end
  end

  describe ".tagged?" do
    it "accepts a well-formed tagged hash" do
      expect(adt.tagged?(adt.make(:K, [1]))).to be(true)
    end

    it "rejects a hash with extra keys" do
      expect(adt.tagged?({ tag: :K, values: [], extra: 1 })).to be(false)
    end

    it "rejects a hash with string tag value" do
      expect(adt.tagged?({ tag: "K", values: [] })).to be(false)
    end

    it "rejects a non-hash" do
      expect(adt.tagged?([:K, []])).to be(false)
      expect(adt.tagged?(nil)).to be(false)
    end
  end

  describe ".match" do
    it "yields tag and values to the block on a tagged hash" do
      called_with = nil
      adt.match(adt.make(:Just, [7])) { |t, vs| called_with = [t, vs] }
      expect(called_with).to eq([:Just, [7]])
    end

    it "raises BoundaryError on a non-ADT input" do
      expect { adt.match({ not: :adt }) { } }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end
  end

  describe ".tag and .values accessors" do
    it "returns the tag and the values of a tagged hash" do
      v = adt.make(:Pair, [:a, :b])
      expect(adt.tag(v)).to eq(:Pair)
      expect(adt.values(v)).to eq([:a, :b])
    end

    it "raises BoundaryError on non-ADT input" do
      expect { adt.tag({}) }.to raise_error(Sapphire::Runtime::Errors::BoundaryError)
      expect { adt.values({}) }.to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end
  end

  describe ".define" do
    it "installs a nullary constructor method on the target module" do
      mod = Module.new
      adt.define(mod, :Red)
      expect(mod.Red).to eq({ tag: :Red, values: [] })
      expect(mod.Red).to be_frozen
    end

    it "installs a positional-arity constructor method" do
      mod = Module.new
      adt.define(mod, :Just, arity: 1)
      expect(mod.Just(99)).to eq({ tag: :Just, values: [99] })
    end

    it "supports higher arity constructors positionally" do
      mod = Module.new
      adt.define(mod, :Triple, arity: 3)
      expect(mod.Triple(1, 2, 3)).to eq({ tag: :Triple, values: [1, 2, 3] })
    end

    it "enforces the declared arity via Ruby's own argument count check" do
      mod = Module.new
      adt.define(mod, :Pair, arity: 2)
      expect { mod.Pair(1) }.to raise_error(ArgumentError)
    end

    it "rejects a non-Module target" do
      expect { adt.define("not a module", :K) }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end

    it "rejects a negative arity" do
      mod = Module.new
      expect { adt.define(mod, :K, arity: -1) }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end
  end

  describe ".define_variants" do
    it "defines multiple nullary variants from a hash" do
      mod = Module.new
      adt.define_variants(mod, { Red: 0, Green: 0, Blue: 0 })
      expect(mod.Red).to eq({ tag: :Red, values: [] })
      expect(mod.Green).to eq({ tag: :Green, values: [] })
      expect(mod.Blue).to eq({ tag: :Blue, values: [] })
    end

    it "defines mixed-arity variants (Maybe-like)" do
      mod = Module.new
      adt.define_variants(mod, { Nothing: 0, Just: 1 })
      expect(mod.Nothing).to eq({ tag: :Nothing, values: [] })
      expect(mod.Just(5)).to eq({ tag: :Just, values: [5] })
    end
  end
end
