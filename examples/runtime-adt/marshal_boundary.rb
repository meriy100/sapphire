# frozen_string_literal: true

# 境界 marshalling のサンプル。`Sapphire::Runtime::Marshal.from_ruby`
# / `to_ruby` が Ruby 側の基本値・レコード・ADT をどう扱うかを示す。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-adt/marshal_boundary.rb

require "sapphire/runtime"

M   = Sapphire::Runtime::Marshal
ADT = Sapphire::Runtime::ADT

# 1) 基本値の往復
puts "Int:    #{M.from_ruby(42).inspect}"
puts "Bool:   #{M.from_ruby(true).inspect}"
puts "String: #{M.from_ruby("hello").inspect}  (frozen? #{M.from_ruby("hello").frozen?})"

# 2) Array は List として通る
puts "List:   #{M.from_ruby([1, 2, 3]).inspect}"

# 3) Record はシンボルキー Hash で
rec = M.from_ruby({ name: "Bob", age: 21 })
puts "Record: #{rec.inspect}  (frozen? #{rec.frozen?})"

# 4) 既製の tagged-hash も from_ruby で frozen 化できる
raw_ok = { tag: :Ok, values: [99] }
ok = M.from_ruby(raw_ok)
puts "ADT(Ok): #{ok.inspect}  (frozen? #{ok.frozen?})"

# 5) サポート外入力は MarshalError
begin
  M.from_ruby(nil)
rescue Sapphire::Runtime::Errors::MarshalError => e
  puts "Rejected nil -> #{e.class.name}: #{e.message}"
end

begin
  M.from_ruby(1.5)
rescue Sapphire::Runtime::Errors::MarshalError => e
  puts "Rejected Float -> #{e.class.name}: #{e.message}"
end

# 6) 文字列キー Hash は record として認められない（10-OQ3）
begin
  M.from_ruby({ "name" => "X" })
rescue Sapphire::Runtime::Errors::MarshalError => e
  puts "Rejected string-keyed Hash -> #{e.class.name}"
end

# 7) Sapphire 値 -> Ruby: 生成コードが境界越しに Ruby に値を渡すケース
just5 = ADT.make(:Just, [5])
puts "to_ruby(Just 5): #{M.to_ruby(just5).inspect}"

puts "OK: boundary marshalling exercised."
