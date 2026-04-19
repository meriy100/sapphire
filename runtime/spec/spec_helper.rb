# frozen_string_literal: true

require "sapphire/runtime"

RSpec.configure do |config|
  config.disable_monkey_patching!

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end

  config.mock_with :rspec do |c|
    c.syntax = :expect
    c.verify_partial_doubles = true
  end

  # Run specs in random order to surface order dependencies.
  config.order = :random
  Kernel.srand config.seed
end
