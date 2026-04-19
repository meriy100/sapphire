# frozen_string_literal: true

# `prim_bind` で effect monad を連鎖させるサンプル。do 記法の脱糖
# （spec 11 §`:=` and `Ruby` — the loop closed）で生成コードが
# 出すことになる形をそのまま手書きしている。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-monad/chained_bind.rb

require "sapphire/runtime"

Rb = Sapphire::Runtime::Ruby

# Sapphire で以下に相当:
#
#   greet : Ruby String
#   greet = do
#     puts "step 1"
#     x := 10
#     y := x * 2
#     pure ("got " ++ show (x + y))
#
action =
  Rb.prim_bind(Rb.prim_embed { puts "step 1"; 0 }) do |_|
    Rb.prim_bind(Rb.prim_embed { 10 }) do |x|
      Rb.prim_bind(Rb.prim_embed { x * 2 }) do |y|
        Rb.prim_return("got #{x + y}")
      end
    end
  end

status, value = Rb.run(action)

puts "status => #{status.inspect}"
puts "value  => #{value.inspect}"

raise "expected :ok" unless status == :ok
raise "expected 'got 30'" unless value == "got 30"

puts "OK: chained bind produced the expected value."
