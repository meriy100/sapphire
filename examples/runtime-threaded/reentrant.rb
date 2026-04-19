# frozen_string_literal: true

# R5 — `Ruby.run` の再入（reentrant）サンプル。
#
# `prim_embed` block の中でさらに `Ruby.run` を呼ぶことは
# admitted（I-OQ47、docs/impl/16-runtime-threaded-loading.md
# §再入）。ネストした `run` はそれぞれ独立な evaluator Thread を
# 起こし、`Thread#value` で join して結果を返す。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-threaded/reentrant.rb

require "sapphire/runtime"

Rb = Sapphire::Runtime::Ruby

outer = Rb.prim_embed do
  outer_thread = Thread.current
  outer_is_main = outer_thread.equal?(Thread.main)

  # 内側の `run` も fresh Thread を起こす。block の戻り値で
  # `Thread` 自体を marshal させると境界を通らないので、比較結果
  # を真偽値（および識別用の文字列）に畳んで返す。
  _, inner_summary = Rb.run(Rb.prim_embed do
    inner_thread = Thread.current
    "inner_is_main=#{inner_thread.equal?(Thread.main)}"
  end)

  "outer_is_main=#{outer_is_main} #{inner_summary}"
end

status, summary = Rb.run(outer)
raise "outer run failed: #{status.inspect}" unless status == :ok

puts summary
unless summary == "outer_is_main=false inner_is_main=false"
  raise "expected both evaluator threads to be non-main; got #{summary.inspect}"
end
puts "OK: reentrant run spawned a distinct Thread (outer) and a further distinct Thread (inner)."

# 内側が raise したケース：`[:err, RubyError]` が inner run から
# 戻るので、outer は plain に [:ok, ...] のまま続行する。
outer_with_inner_failure = Rb.prim_embed do
  inner_status, _ = Rb.run(Rb.prim_embed { raise "inner boom" })
  inner_status.to_s
end

expect_status, expect_value = Rb.run(outer_with_inner_failure)
raise "unexpected" unless expect_status == :ok && expect_value == "err"

puts "OK: inner failure surfaced as [:err, _] to the outer block (not raised)."
