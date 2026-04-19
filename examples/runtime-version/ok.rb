# frozen_string_literal: true

# R6 — `Sapphire::Runtime.require_version!` の成功パス。
#
# 生成コード（I7c）が各 Sapphire モジュールの先頭で
# `Sapphire::Runtime.require_version!("~> x.y")` を呼ぶことで
# コンパイル時と load 時の runtime gem 不整合を早期検出する
# (docs/impl/16-runtime-threaded-loading.md §R6)。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-version/ok.rb

require "sapphire/runtime"

# 現ランタイムは 0.1.0。`~> 0.1` は 0.y.z <1.0 を許すので通る。
loaded = Sapphire::Runtime.require_version!("~> 0.1")

puts "sapphire-runtime VERSION => #{Sapphire::Runtime::VERSION}"
puts "require_version! returned => #{loaded.inspect}"
raise "expected the loaded VERSION" unless loaded == Sapphire::Runtime::VERSION

# 配列形でも同じ意味の制約が書ける。
also_ok = Sapphire::Runtime.require_version!([">= 0.1.0", "< 1.0"])
raise "expected array constraint to also succeed" unless also_ok == Sapphire::Runtime::VERSION

puts "OK: require_version! satisfied."
