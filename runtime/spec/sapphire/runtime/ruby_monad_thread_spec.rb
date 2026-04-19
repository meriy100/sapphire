# frozen_string_literal: true

# R5 — `Ruby.run` thread-isolation spec.
#
# Per docs/spec/11-ruby-monad.md §Execution model and
# docs/impl/16-runtime-threaded-loading.md, `Ruby.run` spawns a
# fresh Ruby `Thread` per invocation, joins it with
# `Thread#value`, and isolates Ruby-side locals / thread-local
# storage across `run`s. Global variables and loaded constants
# are explicitly **not** isolated (see the design doc for the
# rationale); this spec pins both the positive isolation
# properties and the intentional non-isolation ones.

RSpec.describe Sapphire::Runtime::Ruby, "R5 thread isolation" do
  let(:rb) { described_class }

  describe "evaluator thread identity" do
    it "runs prim_embed on a Thread that is not the main Thread" do
      # MRI may recycle `Thread#object_id` after a Thread dies, so
      # comparing raw ids is unreliable across `run` boundaries.
      # `Thread#equal?` / identity comparison through a Thread
      # reference we hold alive is the dependable check.
      action = rb.prim_embed do
        Thread.current.equal?(Thread.main) ? "main" : "other"
      end
      status, tag = rb.run(action)

      expect(status).to eq(:ok)
      expect(tag).to eq("other")
    end

    it "caller and evaluator Thread objects are not the same object" do
      caller_thread = Thread.current
      captured = nil
      action = rb.prim_embed { captured = Thread.current; 0 }
      rb.run(action)

      expect(captured).to be_a(Thread)
      expect(captured.equal?(caller_thread)).to be(false)
    end

    it "each run uses a distinct Thread object" do
      captured1 = nil
      captured2 = nil
      rb.run(rb.prim_embed { captured1 = Thread.current; 0 })
      rb.run(rb.prim_embed { captured2 = Thread.current; 0 })

      expect(captured1).to be_a(Thread)
      expect(captured2).to be_a(Thread)
      expect(captured1.equal?(captured2)).to be(false)
    end

    it "the evaluator thread is not alive after run returns" do
      captured = nil
      action = rb.prim_embed { captured = Thread.current; 0 }
      rb.run(action)

      expect(captured).to be_a(Thread)
      # After `Thread#value` returns, the thread has finished.
      expect(captured.alive?).to be(false)
    end

    it "caller thread keeps running as the same Thread across a run call" do
      before = Thread.current
      rb.run(rb.prim_embed { 1 })
      after = Thread.current

      expect(after.equal?(before)).to be(true)
    end
  end

  describe "thread-local isolation" do
    it "does not leak Thread.current[:key] from caller into the evaluator thread" do
      Thread.current[:sapphire_r5_probe] = :caller_value
      # Boundary rule: `Marshal.from_ruby(nil)` is not admissible,
      # so we return a present/absent string rather than the raw
      # thread-local value.
      action = rb.prim_embed do
        Thread.current[:sapphire_r5_probe].nil? ? "absent" : "present"
      end
      status, inner = rb.run(action)

      expect(status).to eq(:ok)
      expect(inner).to eq("absent")
    ensure
      Thread.current[:sapphire_r5_probe] = nil
    end

    it "does not leak Thread.current[:key] from a run into the caller" do
      Thread.current[:sapphire_r5_leak] = nil
      action = rb.prim_embed { Thread.current[:sapphire_r5_leak] = :inner; 0 }
      rb.run(action)

      expect(Thread.current[:sapphire_r5_leak]).to be_nil
    end

    it "does not share Thread.current[:key] between two sequential runs" do
      rb.run(rb.prim_embed { Thread.current[:sapphire_r5_cross] = :first; 0 })
      action = rb.prim_embed do
        Thread.current[:sapphire_r5_cross].nil? ? "absent" : "present"
      end
      _, seen = rb.run(action)

      expect(seen).to eq("absent")
    end
  end

  describe "reentrant run" do
    it "admits a run invocation nested inside a prim_embed block" do
      outer = rb.prim_embed do
        _, v = rb.run(rb.prim_return(7))
        v + 1
      end

      expect(rb.run(outer)).to eq([:ok, 8])
    end

    it "each reentrant run uses its own fresh evaluator Thread" do
      outer_thread = nil
      inner_thread = nil

      outer = rb.prim_embed do
        outer_thread = Thread.current
        captured_inner = nil
        rb.run(rb.prim_embed { captured_inner = Thread.current; 0 })
        inner_thread = captured_inner
        0
      end
      rb.run(outer)

      expect(outer_thread).to be_a(Thread)
      expect(inner_thread).to be_a(Thread)
      expect(outer_thread.equal?(inner_thread)).to be(false)
      expect(outer_thread.equal?(Thread.main)).to be(false)
      expect(inner_thread.equal?(Thread.main)).to be(false)
    end

    it "propagates the inner run's [:err, _] back to the outer action" do
      outer = rb.prim_embed do
        status, _err = rb.run(rb.prim_embed { raise "inner boom" })
        status.to_s
      end

      # The inner run returns [:err, RubyError] rather than raising,
      # so the outer run sees a plain Ok with the status string.
      expect(rb.run(outer)).to eq([:ok, "err"])
    end

    it "inner run does not pollute the outer evaluator's thread-locals" do
      outer = rb.prim_embed do
        Thread.current[:sapphire_r5_outer] = :outer
        rb.run(rb.prim_embed { Thread.current[:sapphire_r5_outer] = :inner; 0 })
        Thread.current[:sapphire_r5_outer].to_s
      end

      expect(rb.run(outer)).to eq([:ok, "outer"])
    end

    it "propagates Interrupt across nested runs (two-level Thread#value re-raise)" do
      # B-03-OQ5 DECIDED + I-OQ47 DECIDED: a signal raised inside
      # the inner evaluator Thread must re-raise on the inner
      # `run`'s caller (the outer evaluator Thread), which in turn
      # lets it escape the outer action back to `Thread#value`'s
      # re-raise on the original caller. The two-level Thread#value
      # chain is what keeps propagation intact.
      #
      # SystemExit is covered by the top-level "does not rescue
      # SystemExit" test but deliberately not re-tested here: a
      # SystemExit propagating through the rspec runner perturbs
      # its subsequent output buffer under random ordering (rspec
      # catches it but `at_exit` finalisation interleaves with the
      # reporter). Interrupt suffices to pin the two-level
      # Thread#value chain for the nested-run case.
      inner = rb.prim_embed { raise Interrupt }
      outer = rb.prim_embed { rb.run(inner) }
      expect { rb.run(outer) }.to raise_error(Interrupt)
    end
  end

  describe "StandardError propagation via Thread#value" do
    it "converts a StandardError raised inside the evaluator thread to [:err, RubyError]" do
      action = rb.prim_embed { raise StandardError, "threaded boom" }
      status, err = rb.run(action)

      expect(status).to eq(:err)
      expect(Sapphire::Runtime::ADT.tag(err)).to eq(:RubyError)
      class_name, message, _ = Sapphire::Runtime::ADT.values(err)
      expect(class_name).to eq("StandardError")
      expect(message).to eq("threaded boom")
    end

    it "short-circuits a bind chain inside the evaluator thread" do
      ran_after = false
      action = rb.prim_bind(rb.prim_embed { raise "halt" }) do |_|
        ran_after = true
        rb.prim_return(:unreached)
      end
      status, _ = rb.run(action)

      expect(status).to eq(:err)
      expect(ran_after).to be(false)
    end
  end

  describe "Interrupt / SystemExit propagation" do
    # B-03-OQ5 DECIDED: system-level exceptions propagate past the
    # boundary. With R5 they are raised inside the evaluator
    # thread; `Thread#value` must re-raise them on the caller side
    # so the surface behaviour is unchanged.

    it "does not rescue Interrupt raised inside the evaluator thread" do
      action = rb.prim_embed { raise Interrupt }
      expect { rb.run(action) }.to raise_error(Interrupt)
    end

    it "does not rescue SystemExit raised inside the evaluator thread" do
      action = rb.prim_embed { raise SystemExit }
      expect { rb.run(action) }.to raise_error(SystemExit)
    end

    it "does not rescue a Signal raised inside the evaluator thread" do
      action = rb.prim_embed { raise SignalException, "SIGTERM" }
      expect { rb.run(action) }.to raise_error(SignalException)
    end

    it "the re-raised signal is the original class (not RuntimeError)" do
      action = rb.prim_embed { raise Interrupt, "ctrl-c" }
      begin
        rb.run(action)
        raise "expected Interrupt to propagate"
      rescue Interrupt => e
        expect(e).to be_a(Interrupt)
        expect(e.message).to eq("ctrl-c")
      end
    end
  end

  describe "blocking semantics" do
    it "blocks the caller until the evaluator thread completes" do
      start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      action = rb.prim_embed { sleep 0.05; 42 }
      _, value = rb.run(action)
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - start

      expect(value).to eq(42)
      expect(elapsed).to be >= 0.04
    end

    it "runs two sequential run calls in order (no interleaving)" do
      log = []
      action_a = rb.prim_embed { log << :a_start; sleep 0.02; log << :a_end; 1 }
      action_b = rb.prim_embed { log << :b_start; sleep 0.02; log << :b_end; 2 }

      rb.run(action_a)
      rb.run(action_b)

      expect(log).to eq(%i[a_start a_end b_start b_end])
    end
  end

  describe "local-scope isolation" do
    # Spec 11 §Execution model item 4: per-step fresh Ruby local
    # scope. The Thread-level isolation does not weaken this (if
    # anything it strengthens it).

    it "locals set in one prim_embed block are not visible in a later one" do
      step1 = rb.prim_embed do
        # Ruby block-locals die with the block
        leaked = 123
        leaked
      end
      step2 = rb.prim_embed do
        defined?(leaked) ? "seen" : "fresh"
      end
      chain = rb.prim_bind(step1) { |_| step2 }
      expect(rb.run(chain)).to eq([:ok, "fresh"])
    end

    it "captured closures still see their lexical captures" do
      # Closure capture (a local from the surrounding Ruby method
      # scope) is orthogonal to per-snippet scope isolation.
      captured = 9
      action = rb.prim_embed { captured * 2 }
      expect(rb.run(action)).to eq([:ok, 18])
    end
  end

  describe "concurrent callers" do
    # Two caller threads calling `Ruby.run` simultaneously must
    # each get their own evaluator thread. There is no shared
    # mutable runtime state between them.
    it "two caller threads each get independent evaluator threads" do
      # Four caller Threads each `Ruby.run` concurrently; each
      # evaluator Thread must be a distinct object and none may be
      # the main Thread. We capture the evaluator `Thread` inside
      # the block (rather than its `object_id`, which MRI may
      # recycle once a Thread dies) and compare by identity.
      captured_mutex = Mutex.new
      captured = []

      threads = Array.new(4) do
        Thread.new do
          local = nil
          # Hold a reference long enough for cross-thread identity
          # comparison to be meaningful (the Thread will not be
          # GC'd while `local` keeps it reachable).
          rb.run(rb.prim_embed do
            local = Thread.current
            sleep 0.005
            0
          end)
          captured_mutex.synchronize { captured << local }
        end
      end
      threads.each(&:join)

      expect(captured.size).to eq(4)
      expect(captured).to all(be_a(Thread))
      # All four must be distinct Thread objects.
      captured.each_with_index do |t, i|
        captured.each_with_index do |u, j|
          next if i == j

          expect(t.equal?(u)).to be(false)
        end
      end
      # None of them is the main Thread.
      captured.each do |t|
        expect(t.equal?(Thread.main)).to be(false)
      end
    end
  end
end
