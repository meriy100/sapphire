# frozen_string_literal: true

# R4 / R5 — `Sapphire::Runtime::RubyError` direct-unit spec.
#
# `RubyError.from_exception` is exercised transitively through
# `Sapphire::Runtime::Ruby.run`'s boundary rescue in
# `ruby_monad_spec.rb` / `ruby_monad_thread_spec.rb`, but the
# string-sanitisation branch set (`scrub` / `encode` / pass-through)
# is easier to pin against regression with direct unit tests. This
# file covers those three paths plus the `from_exception` top-level
# contract on payloads the boundary rescue would not normally reach
# (non-ASCII messages, pre-forced `BINARY` encodings, etc.).

RSpec.describe Sapphire::Runtime::RubyError, "sanitize_string branches" do
  let(:mod) { described_class }

  # `sanitize_string` is `private_class_method` per the R4 design
  # (internal helper, not boundary surface). The test reaches it
  # via `.send` deliberately — black-box coverage of a private
  # helper in a documented module is standard rspec practice and
  # is worth the access for pinning encoding behaviour.
  def sanitize(s)
    mod.send(:sanitize_string, s)
  end

  describe "UTF-8 valid input" do
    it "passes an ASCII string through unchanged in value" do
      out = sanitize("hello")
      expect(out).to eq("hello")
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to be_frozen
    end

    it "passes a non-ASCII UTF-8 string (Japanese) through unchanged" do
      src = "こんにちは"
      out = sanitize(src)
      expect(out).to eq(src)
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to be_frozen
    end
  end

  describe "UTF-8 invalid input" do
    it "scrubs invalid byte sequences with '?'" do
      # 0xff 0xfe is not a valid UTF-8 sequence. Tagged as UTF-8
      # so the `str.encoding == UTF_8` branch runs `scrub`.
      bad = "\xff\xfe".dup.force_encoding(Encoding::UTF_8)
      expect(bad.valid_encoding?).to be(false)

      out = sanitize(bad)
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to be_valid_encoding
      expect(out).to include("?")
      expect(out).to be_frozen
    end
  end

  describe "non-UTF-8 encodings" do
    it "retags ASCII-8BIT bytes that happen to be valid UTF-8 as UTF-8" do
      # Bytes of "hi" are valid UTF-8 even though encoding-tagged
      # as ASCII-8BIT. The `encode(UTF_8, ...)` branch converts
      # the tag while preserving the content.
      src = "hi".b
      expect(src.encoding).to eq(Encoding::ASCII_8BIT)

      out = sanitize(src)
      expect(out).to eq("hi")
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to be_frozen
    end

    it "replaces undefined bytes from BINARY with '?'" do
      # 0xff is not representable as UTF-8. With `undef: :replace,
      # replace: "?"` the byte becomes "?", so the produced
      # string is valid UTF-8 and contains at least one "?".
      src = "\xff\x80".b
      expect(src.encoding).to eq(Encoding::ASCII_8BIT)

      out = sanitize(src)
      expect(out.encoding).to eq(Encoding::UTF_8)
      expect(out).to be_valid_encoding
      expect(out).to include("?")
      expect(out).to be_frozen
    end
  end
end

RSpec.describe Sapphire::Runtime::RubyError, ".from_exception" do
  it "preserves a Japanese message unchanged through sanitize_string" do
    # UTF-8 valid Japanese is in the pass-through branch of
    # `sanitize_string`, so the message must round-trip verbatim.
    err = StandardError.new("エラーが発生しました")
    adt = described_class.from_exception(err)

    expect(Sapphire::Runtime::ADT.tag(adt)).to eq(:RubyError)
    class_name, message, backtrace = Sapphire::Runtime::ADT.values(adt)
    expect(class_name).to eq("StandardError")
    expect(message).to eq("エラーが発生しました")
    expect(message.encoding).to eq(Encoding::UTF_8)
    expect(message).to be_frozen
    expect(backtrace).to eq([])
    expect(backtrace).to be_frozen
  end
end
