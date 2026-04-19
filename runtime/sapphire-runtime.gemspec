# frozen_string_literal: true

require_relative "lib/sapphire/runtime/version"

Gem::Specification.new do |spec|
  spec.name          = "sapphire-runtime"
  spec.version       = Sapphire::Runtime::VERSION
  spec.authors       = ["meriy100"]
  spec.email         = ["kouta@meriy100.com"]

  spec.summary       = "Ruby-side runtime support library for Sapphire-compiled programs."
  spec.description   = <<~DESC
    sapphire-runtime is the Ruby gem that every Sapphire-compiled
    program depends on at run time. It implements the tagged-hash
    ADT helpers, the `Ruby` monad evaluator (`pure` / `bind` /
    `run`), boundary-exception catching that produces `RubyError`
    values, and the type-directed marshalling helpers between
    Sapphire values and Ruby values. The normative contract lives
    in docs/build/03-sapphire-runtime.md in the Sapphire
    repository.
  DESC
  spec.homepage      = "https://github.com/meriy100/sapphire"
  spec.license       = "MIT"

  # Matches docs/build/03-sapphire-runtime.md §Gem identity
  # (`~> 3.3`, per 01 OQ 1). Equivalent to `>= 3.3, < 4.0`.
  spec.required_ruby_version = "~> 3.3"

  spec.metadata["homepage_uri"]    = spec.homepage
  spec.metadata["bug_tracker_uri"] = "#{spec.homepage}/issues"

  spec.files = Dir[
    "lib/**/*.rb",
    "README.md",
    "sapphire-runtime.gemspec"
  ]
  spec.require_paths = ["lib"]

  spec.add_development_dependency "rspec", "~> 3.13"
end
