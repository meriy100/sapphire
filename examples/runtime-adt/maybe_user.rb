# frozen_string_literal: true

# `Maybe (User { name: String, age: Int })` 相当を Ruby 側で組み立て、
# `case` 文でパターンマッチし、record フィールドを取り出すサンプル。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-adt/maybe_user.rb

require "sapphire/runtime"

module Maybe
end

Sapphire::Runtime::ADT.define(Maybe, :Nothing)
Sapphire::Runtime::ADT.define(Maybe, :Just, arity: 1)

# User は record 型: spec 10 §Records よりシンボルキー Hash で表す。
alice = { name: "Alice", age: 30 }.freeze

found    = Maybe.Just(alice)
notfound = Maybe.Nothing

def describe(opt)
  case opt[:tag]
  when :Just
    user = opt[:values][0]
    "Just user: name=#{user[:name]} age=#{user[:age]}"
  when :Nothing
    "Nothing"
  else
    raise "unknown tag: #{opt[:tag]}"
  end
end

puts describe(found)
puts describe(notfound)

# 構造的等価により、同一構成の `Just` 同士は == で一致する。
another_found = Maybe.Just({ name: "Alice", age: 30 })
raise "Just values with equal payload should be equal" unless found == another_found

puts "OK: Maybe + User record round-tripped via tagged hash."
