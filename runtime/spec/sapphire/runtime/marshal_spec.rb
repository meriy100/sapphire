# frozen_string_literal: true

RSpec.describe Sapphire::Runtime::Marshal do
  let(:m) { described_class }

  describe "ground types" do
    it "round-trips Integer" do
      expect(m.from_ruby(42)).to eq(42)
      expect(m.to_ruby(42)).to eq(42)
    end

    it "round-trips arbitrary-precision Integer" do
      big = 10**50
      expect(m.from_ruby(big)).to eq(big)
    end

    it "round-trips Boolean" do
      expect(m.from_ruby(true)).to eq(true)
      expect(m.from_ruby(false)).to eq(false)
      expect(m.to_ruby(true)).to eq(true)
      expect(m.to_ruby(false)).to eq(false)
    end

    it "rejects Float (07-OQ6)" do
      expect { m.from_ruby(1.5) }.to raise_error(Sapphire::Runtime::Errors::MarshalError)
      expect { m.to_ruby(1.5) }.to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "String" do
    it "keeps UTF-8 strings intact and returns them frozen" do
      s = m.from_ruby("hello")
      expect(s).to eq("hello")
      expect(s).to be_frozen
      expect(s.encoding).to eq(Encoding::UTF_8)
    end

    it "re-encodes an ASCII-8BIT string that is valid UTF-8" do
      raw = +"ok"
      raw.force_encoding(Encoding::ASCII_8BIT)
      out = m.from_ruby(raw)
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to eq("ok")
    end

    it "rejects a string that is not valid UTF-8" do
      bad = (+"\xff\xfe").force_encoding(Encoding::UTF_8)
      expect { m.from_ruby(bad) }.to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "Array (List)" do
    it "marshals an empty array" do
      out = m.from_ruby([])
      expect(out).to eq([])
      expect(out).to be_frozen
    end

    it "marshals recursively" do
      out = m.from_ruby([1, 2, 3])
      expect(out).to eq([1, 2, 3])
    end

    it "propagates MarshalError from an element" do
      expect { m.from_ruby([1, 1.5]) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "records (symbol-keyed hashes)" do
    it "accepts a symbol-keyed Hash as a record" do
      out = m.from_ruby({ name: "Alice", age: 30 })
      expect(out).to eq({ name: "Alice", age: 30 })
      expect(out).to be_frozen
    end

    it "accepts the empty record {}" do
      out = m.from_ruby({})
      expect(out).to eq({})
      expect(out).to be_frozen
    end

    it "rejects a string-keyed Hash (symbol-keyed contract, 10-OQ3)" do
      expect { m.from_ruby({ "name" => "Alice" }) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "tagged ADT hashes" do
    it "rebuilds frozen ADT values from a well-formed tagged hash" do
      out = m.from_ruby({ tag: :Just, values: [7] })
      expect(out).to eq({ tag: :Just, values: [7] })
      expect(out).to be_frozen
      expect(out[:values]).to be_frozen
    end

    it "recurses into nested ADTs" do
      nested = { tag: :Just, values: [{ tag: :Just, values: [1] }] }
      out = m.from_ruby(nested)
      expect(out).to eq(nested)
    end
  end

  describe "Ordering symbols" do
    it "admits :lt / :eq / :gt" do
      expect(m.from_ruby(:lt)).to eq(:lt)
      expect(m.from_ruby(:eq)).to eq(:eq)
      expect(m.from_ruby(:gt)).to eq(:gt)
    end

    it "rejects any other symbol" do
      expect { m.from_ruby(:hello) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "rejection cases" do
    it "rejects nil (10-OQ1)" do
      expect { m.from_ruby(nil) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
      expect { m.to_ruby(nil) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end

    it "rejects an arbitrary Ruby object" do
      expect { m.from_ruby(Object.new) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end
  end

  describe "to_ruby idempotence" do
    it "returns the same Sapphire-side Integer unchanged" do
      expect(m.to_ruby(42)).to eq(42)
    end

    it "returns a frozen copy of a String" do
      s = +"abc"
      out = m.to_ruby(s)
      expect(out).to eq("abc")
      expect(out).to be_frozen
    end

    it "returns the same tagged ADT (re-frozen)" do
      v = Sapphire::Runtime::ADT.make(:Ok, [1])
      out = m.to_ruby(v)
      expect(out).to eq(v)
      expect(out).to be_frozen
    end

    it "returns the same record (frozen)" do
      r = { name: "Alice", age: 30 }
      out = m.to_ruby(r)
      expect(out).to eq(r)
    end
  end

  describe ".symbol_keyed?" do
    it "is true for a hash with only symbol keys" do
      expect(m.symbol_keyed?({ a: 1, b: 2 })).to be(true)
    end

    it "is true for the empty hash" do
      expect(m.symbol_keyed?({})).to be(true)
    end

    it "is false if any key is a String" do
      expect(m.symbol_keyed?({ "a" => 1 })).to be(false)
    end

    it "is false for a non-Hash" do
      expect(m.symbol_keyed?(nil)).to be(false)
    end
  end
end
