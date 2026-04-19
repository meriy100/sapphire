# frozen_string_literal: true

# `Ruby a` monad で `puts "hello"` 相当の副作用を最小手で走らせる
# サンプル。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-monad/hello_ruby.rb

require "sapphire/runtime"

Rb = Sapphire::Runtime::Ruby

# `puts "hello"` を遅延評価の `Ruby {}` 値として作る。`prim_embed`
# は Ruby スニペット（spec 10 §The embedding form、`:=` 束縛の
# コンパイル結果に相当）を effect monad 値に包む。
action = Rb.prim_embed do
  puts "hello"
  # `:=` 束縛の結果型が `Ruby {}` のとき、Ruby 側は空レコード `{}`
  # を返す（spec 10 §Records）。
  {}
end

# `run` がここで走る。戻り値は `[:ok, value]` / `[:err, err]` の
# タプル（`Result RubyError a` に対応、spec 11 §run）。
result = Rb.run(action)

puts "result => #{result.inspect}"
raise "expected :ok" unless result[0] == :ok
raise "expected {}" unless result[1] == {}

puts "OK: hello_ruby action evaluated."
