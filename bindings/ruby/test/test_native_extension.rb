# frozen_string_literal: true

require "minitest/autorun"
require "rbconfig"

class TestNativeExtension < Minitest::Test
  def test_platform_native_extension_exists
    ext = RbConfig::CONFIG.fetch("DLEXT")
    native = File.expand_path("../lib/ptiff.#{ext}", __dir__)
    assert File.exist?(native), "expected native extension at #{native}"
  end
end
