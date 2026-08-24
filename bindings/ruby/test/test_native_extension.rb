# frozen_string_literal: true

require 'minitest/autorun'
require 'rbconfig'
require 'tmpdir'
require_relative '../lib/ptiff/native_extension'

class TestNativeExtension < Minitest::Test
  def test_prefers_platform_native_extension_suffix
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, 'ptiff.bundle'), '')
      native = File.join(dir, "ptiff.#{RbConfig::CONFIG.fetch('DLEXT')}")
      File.write(native, '')

      assert_equal native, PTiff::NativeExtension.find(dir)
    end
  end

  def test_falls_back_to_bundle_when_platform_suffix_is_unavailable
    Dir.mktmpdir do |dir|
      bundle = File.join(dir, 'ptiff.bundle')
      File.write(bundle, '')

      assert_equal bundle, PTiff::NativeExtension.find(dir, { 'DLEXT' => 'missing', 'DLEXT2' => nil })
    end
  end
end
