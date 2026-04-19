# frozen_string_literal: true

# R6 — `Sapphire::Runtime.require_version!` の失敗パス。
#
# 意図的に現ランタイム (0.1.0) を満たさない `~> 99.0` を要求し、
# `Sapphire::Runtime::Errors::RuntimeVersionMismatch` が
# raise されることを確認する。実際の生成コードでは、コンパイラが
# 埋め込んだ制約を load 時に satisfy できないとここと同じ
# エラーが出る想定。
#
# 実行:
#   ruby -I runtime/lib examples/runtime-version/mismatch.rb

require "sapphire/runtime"

begin
  Sapphire::Runtime.require_version!("~> 99.0")
  raise "expected RuntimeVersionMismatch, nothing raised"
rescue Sapphire::Runtime::Errors::RuntimeVersionMismatch => e
  puts "caught RuntimeVersionMismatch as expected:"
  puts "  message => #{e.message}"

  unless e.message.include?(Sapphire::Runtime::VERSION)
    raise "error message must name the loaded VERSION"
  end
  unless e.message.include?("99")
    raise "error message must name the required constraint"
  end
end

# 構文エラーは LoadError になる（version 不整合とは区別）。
begin
  Sapphire::Runtime.require_version!("not a version")
  raise "expected LoadError, nothing raised"
rescue Sapphire::Runtime::Errors::LoadError => e
  puts "caught LoadError for malformed constraint:"
  puts "  message => #{e.message}"
end

puts "OK: mismatch and malformed constraints both raise the expected runtime errors."
