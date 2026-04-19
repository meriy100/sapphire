# frozen_string_literal: true

# R5 ランタイムスレッド分離のサンプル。
#
# `Ruby.run` は spec 11 §Execution model 項 1 に従って **fresh
# Ruby evaluator thread** を `run` ごとに起こす（詳細は
# docs/impl/16-runtime-threaded-loading.md）。
# このサンプルでは caller thread の `Thread.object_id` と、
# `prim_embed` block 内で観測した `Thread.object_id` が
# 一致しないことを確認する。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-threaded/main.rb

require "sapphire/runtime"

Rb = Sapphire::Runtime::Ruby

# NOTE: MRI (CRuby) は Thread オブジェクトの object_id を、thread
# が dead 化したあと別の Thread に再利用することがある（値として
# 観測すると main thread と衝突して見えることがある）。ここでは
# `Thread#equal?` で **同一オブジェクトかどうか** を見る。
puts "caller thread is main? #{Thread.current.equal?(Thread.main)}"

action = Rb.prim_embed do
  # この block は `run` が起こした evaluator Thread 上で走る。
  # Thread オブジェクト自体は境界を通せないので、主要な観測結果を
  # 文字列に畳んで返す。
  t = Thread.current
  "eval_is_main=#{t.equal?(Thread.main)}"
end

status, summary = Rb.run(action)
raise "expected :ok, got #{status.inspect}" unless status == :ok

puts "eval summary:  #{summary}"
unless summary == "eval_is_main=false"
  raise "R5 thread isolation broken: evaluator is the main thread"
end

puts "OK: run spawned a distinct Thread from the main thread."

# もう一度走らせると、また別の Thread が起こされる（プールは
# していない、B-03-OQ4 draft: fresh per run）。生きているあいだ
# だけ Thread を local 変数に握って equal? で比較する方法もある
# が、ここでは caller-side で別々の run が互いに共有する
# `Thread.current[:...]` を見て、間接的に fresh であることを
# 観測する。
_, first = Rb.run(Rb.prim_embed { Thread.current[:sapphire_seen] = "first"; "ok" })
_, second_seen = Rb.run(Rb.prim_embed do
  Thread.current[:sapphire_seen].nil? ? "absent" : "present"
end)
raise "first run did not write thread-local" unless first == "ok"
unless second_seen == "absent"
  raise "second run saw first run's Thread.current[:sapphire_seen]; thread pool leak"
end
puts "OK: second run had a clean Thread.current (fresh Thread per run)."
