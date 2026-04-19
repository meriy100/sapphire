# frozen_string_literal: true

# Ruby 側で `raise` された例外が effect monad の境界で `RubyError`
# に包まれ、`[:err, e]` で返ることを確認するサンプル。spec 10
# §Exception model と spec 11 §run の振る舞いを最小手で示す。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-monad/failure.rb

require "sapphire/runtime"

Rb = Sapphire::Runtime::Ruby

# 最初の step で raise する chain。以後の bind は spec 11
# §Execution model item 5 により short-circuit されて実行されない。
reached_second = false
action =
  Rb.prim_bind(Rb.prim_embed { raise ArgumentError, "bad input" }) do |_|
    reached_second = true
    Rb.prim_return(:never)
  end

status, err = Rb.run(action)

puts "status => #{status.inspect}"
puts "err    => #{err.inspect}"

raise "expected :err" unless status == :err

# `err` は `{ tag: :RubyError, values: [class_name, message,
# backtrace] }` の frozen なタグ付きハッシュ（spec 10 §Exception
# model）。ADT ヘルパでフィールドを取り出して表示する。
class_name, message, backtrace = Sapphire::Runtime::ADT.values(err)

puts "class_name => #{class_name.inspect}"
puts "message    => #{message.inspect}"
puts "backtrace  => (#{backtrace.size} entries)"

raise "unexpected class_name" unless class_name == "ArgumentError"
raise "unexpected message"    unless message == "bad input"
raise "backtrace must be a List String (Array<String>)" unless backtrace.all? { |l| l.is_a?(String) }
raise "second step should have been skipped" if reached_second

puts "OK: failure action short-circuited and produced a RubyError."
