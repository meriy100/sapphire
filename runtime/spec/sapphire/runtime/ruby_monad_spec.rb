# frozen_string_literal: true

RSpec.describe Sapphire::Runtime::Ruby do
  let(:rb) { described_class }

  describe ".prim_return and .run" do
    it "runs a pure value and returns [:ok, value]" do
      expect(rb.run(rb.prim_return(42))).to eq([:ok, 42])
    end

    it "preserves Sapphire-side strings unchanged" do
      out = rb.run(rb.prim_return("hello"))
      expect(out).to eq([:ok, "hello"])
    end

    it "round-trips a tagged-hash ADT value" do
      v = Sapphire::Runtime::ADT.make(:Just, [7])
      expect(rb.run(rb.prim_return(v))).to eq([:ok, v])
    end
  end

  describe ".prim_bind" do
    it "composes prim_return with a continuation" do
      action = rb.prim_bind(rb.prim_return(1)) { |n| rb.prim_return(n + 1) }
      expect(rb.run(action)).to eq([:ok, 2])
    end

    it "sequences three binds in order" do
      action = rb.prim_bind(rb.prim_return(1)) do |a|
        rb.prim_bind(rb.prim_return(a + 10)) do |b|
          rb.prim_bind(rb.prim_return(b + 100)) do |c|
            rb.prim_return([a, b, c])
          end
        end
      end
      expect(rb.run(action)).to eq([:ok, [1, 11, 111]])
    end

    it "threads the continuation's result through the next bind" do
      log = []
      action = rb.prim_bind(rb.prim_embed { log << :first; 10 }) do |x|
        rb.prim_bind(rb.prim_embed { log << :second; x * 2 }) do |y|
          rb.prim_return(y + 1)
        end
      end
      expect(rb.run(action)).to eq([:ok, 21])
      expect(log).to eq([:first, :second])
    end

    it "raises BoundaryError if the upstream argument is not an Action" do
      expect { rb.prim_bind(42) { |x| rb.prim_return(x) } }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end

    it "raises BoundaryError when called without a block" do
      expect { rb.prim_bind(rb.prim_return(1)) }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end

    it "surfaces a continuation whose result is not an Action as [:err, RubyError]" do
      # Per docs/build/03-sapphire-runtime.md §Errors namespace,
      # a runtime BoundaryError raised inside a running action is
      # repackaged as a RubyError by `run`, just like any other
      # StandardError. It does not escape the boundary.
      bad = rb.prim_bind(rb.prim_return(1)) { |n| n + 1 }
      status, err = rb.run(bad)
      expect(status).to eq(:err)
      class_name, _message, _bt = Sapphire::Runtime::ADT.values(err)
      expect(class_name).to eq("Sapphire::Runtime::Errors::BoundaryError")
    end
  end

  describe ".prim_embed" do
    it "wraps a Ruby-side block and returns its marshalled value on run" do
      action = rb.prim_embed { 42 }
      expect(rb.run(action)).to eq([:ok, 42])
    end

    it "defers evaluation until run (no side-effect at construction)" do
      called = false
      action = rb.prim_embed { called = true; :nil_sentinel }
      expect(called).to be(false)
      # The sentinel is not a boundary-admissible symbol, but we
      # only care here that the block did not fire before `run`.
      # Replace the sentinel with a boundary-legal value for the
      # actual run:
      fresh = rb.prim_embed { called = true; 0 }
      expect(rb.run(fresh)).to eq([:ok, 0])
      expect(called).to be(true)
      _ = action # keep the unrun action alive so rubocop-style linters do not warn
    end

    it "raises BoundaryError without a block" do
      expect { rb.prim_embed }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end

    it "marshals the block's return value through Marshal.from_ruby" do
      # The block returns a plain Ruby Hash with symbol keys
      # (a record per spec 10 §Records); the marshal boundary
      # freezes it, and `run` yields the frozen Sapphire-side
      # record.
      action = rb.prim_embed { { name: "Alice", age: 30 } }
      expect(rb.run(action)).to eq([:ok, { name: "Alice", age: 30 }])
      _, payload = rb.run(action)
      expect(payload).to be_frozen
    end

    it "surfaces marshal errors as [:err, RubyError] (Float is rejected)" do
      action = rb.prim_embed { 1.5 }
      status, err = rb.run(action)
      expect(status).to eq(:err)
      expect(Sapphire::Runtime::ADT.tag(err)).to eq(:RubyError)
      _class_name, message, _bt = Sapphire::Runtime::ADT.values(err)
      expect(message).to include("Float")
    end
  end

  describe "exception boundary" do
    it "converts a StandardError raised inside prim_embed to [:err, RubyError]" do
      action = rb.prim_embed { raise StandardError, "boom" }
      status, err = rb.run(action)
      expect(status).to eq(:err)
      expect(Sapphire::Runtime::ADT.tagged?(err)).to be(true)
      expect(Sapphire::Runtime::ADT.tag(err)).to eq(:RubyError)
      class_name, message, backtrace = Sapphire::Runtime::ADT.values(err)
      expect(class_name).to eq("StandardError")
      expect(message).to eq("boom")
      expect(backtrace).to be_a(Array)
      expect(backtrace).to all(be_a(String))
    end

    it "converts a subclass of StandardError (RuntimeError) at the boundary" do
      action = rb.prim_embed { raise "oh no" }
      status, err = rb.run(action)
      expect(status).to eq(:err)
      class_name, message, _bt = Sapphire::Runtime::ADT.values(err)
      expect(class_name).to eq("RuntimeError")
      expect(message).to eq("oh no")
    end

    it "does not rescue Interrupt (system-level, per spec 10 §Exception model)" do
      action = rb.prim_embed { raise Interrupt }
      expect { rb.run(action) }.to raise_error(Interrupt)
    end

    it "short-circuits subsequent binds after a raise (spec 11 §Execution model item 5)" do
      ran_second = false
      action = rb.prim_bind(rb.prim_embed { raise "first" }) do |_|
        ran_second = true
        rb.prim_return(:never)
      end
      status, _err = rb.run(action)
      expect(status).to eq(:err)
      expect(ran_second).to be(false)
    end
  end

  describe "opacity of action values" do
    it "is not treated as a tagged ADT" do
      action = rb.prim_return(1)
      expect(Sapphire::Runtime::ADT.tagged?(action)).to be(false)
    end

    it "prim_embed actions are not tagged ADTs either" do
      action = rb.prim_embed { 1 }
      expect(Sapphire::Runtime::ADT.tagged?(action)).to be(false)
    end

    it "bind actions are not tagged ADTs either" do
      action = rb.prim_bind(rb.prim_return(1)) { |n| rb.prim_return(n) }
      expect(Sapphire::Runtime::ADT.tagged?(action)).to be(false)
    end

    it "is frozen" do
      expect(rb.prim_return(1)).to be_frozen
      expect(rb.prim_embed { 1 }).to be_frozen
      expect(rb.prim_bind(rb.prim_return(1)) { |n| rb.prim_return(n) }).to be_frozen
    end

    it "is refused by Marshal.from_ruby (actions do not cross the boundary as data)" do
      action = rb.prim_return(1)
      expect { Sapphire::Runtime::Marshal.from_ruby(action) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end

    it "is refused by Marshal.to_ruby as well" do
      action = rb.prim_return(1)
      expect { Sapphire::Runtime::Marshal.to_ruby(action) }
        .to raise_error(Sapphire::Runtime::Errors::MarshalError)
    end

    it "has an inspect that hides closure internals" do
      action = rb.prim_embed { :lt }
      expect(action.inspect).to include("Sapphire::Runtime::Ruby::Action")
      expect(action.inspect).to include("kind=embed")
    end

    it "run refuses a non-Action argument" do
      expect { rb.run(:not_an_action) }
        .to raise_error(Sapphire::Runtime::Errors::BoundaryError)
    end
  end

  describe "monad laws (operational)" do
    # The three Haskell-style monad laws on the nose, exercising
    # that `prim_return` / `prim_bind` are wired per spec 11
    # §Class instances.
    #
    # 1. Left identity:  prim_return(a) >>= f  ≡  f(a)
    # 2. Right identity: m >>= prim_return    ≡  m
    # 3. Associativity:  (m >>= f) >>= g      ≡  m >>= (\x -> f x >>= g)
    it "left identity: prim_return(a) >>= f equals f(a)" do
      f = ->(n) { rb.prim_return(n * 3) }
      lhs = rb.prim_bind(rb.prim_return(5), &f)
      rhs = f.call(5)
      expect(rb.run(lhs)).to eq(rb.run(rhs))
    end

    it "right identity: m >>= prim_return equals m" do
      m = rb.prim_return(7)
      lhs = rb.prim_bind(m) { |x| rb.prim_return(x) }
      expect(rb.run(lhs)).to eq(rb.run(m))
    end

    it "associativity: (m >>= f) >>= g equals m >>= (\\x -> f x >>= g)" do
      m = rb.prim_return(2)
      f = ->(n) { rb.prim_return(n + 1) }
      g = ->(n) { rb.prim_return(n * 10) }

      left  = rb.prim_bind(rb.prim_bind(m, &f), &g)
      right = rb.prim_bind(m) { |x| rb.prim_bind(f.call(x), &g) }

      expect(rb.run(left)).to eq(rb.run(right))
    end
  end
end
