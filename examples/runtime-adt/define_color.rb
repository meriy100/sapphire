# frozen_string_literal: true

# `Color = Red | Green | Blue` のような単純な ADT を
# Sapphire::Runtime::ADT の DSL で定義して、生成したコンストラクタを
# 呼び、frozen 性と構造的等価性を確認するサンプル。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-adt/define_color.rb

require "sapphire/runtime"

module Color
end

# 3 つの nullary コンストラクタを一括定義。
Sapphire::Runtime::ADT.define_variants(Color, { Red: 0, Green: 0, Blue: 0 })

red1 = Color.Red
red2 = Color.Red
green = Color.Green

puts "Color.Red   => #{red1.inspect}"
puts "Color.Green => #{green.inspect}"
puts "Color.Blue  => #{Color.Blue.inspect}"

# 値は frozen。ADT ハッシュは不変。
raise "expected frozen" unless red1.frozen?
raise "expected frozen values" unless red1[:values].frozen?

# 構造的等価。同じタグ・同じフィールドなら == が真。
raise "Red should equal Red" unless red1 == red2
raise "Red should differ from Green" if red1 == green

puts "OK: Color ADT defined, frozen, structurally equal."
