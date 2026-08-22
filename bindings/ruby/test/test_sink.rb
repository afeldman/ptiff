# frozen_string_literal: true

# ptiff_sink error-path coverage over the raw SWIG-generated Ruby binding,
# complementing the happy-path round trip in test_roundtrip.rb.
#
# Mirrors bindings/swig/go/sink_test.go and bindings/swig/python/test_sink.py.
# Not ported: double-close / write-after-close -- the raw SWIG handle has no
# close-once guard (that is hand-written-binding sugar), so a second
# ptiff_sink_close is a C-level double-free and writing through an
# already-closed pointer is a use-after-free. Neither is safe to exercise
# against the real C ABI.
#
# Like Python (and unlike Go), a NULL ptiff_sink* surfaces as plain Ruby nil
# here (SWIG's default opaque-pointer typemap for Ruby), so `sink.nil?` is a
# correct, no-typemap-needed NULL check.
require 'minitest/autorun'
require 'tmpdir'
require 'ptiff'

def tile_descriptor
  desc = Ptiff::Ptiff_image_descriptor.new
  desc.width = 32
  desc.height = 32
  desc.pixel_type = Ptiff::PTIFF_PIXEL_UINT8
  desc.channel_count = 1
  desc.has_tile_info = 1
  desc.tile_info.tile_width = 16
  desc.tile_info.tile_height = 16
  desc
end

class TestSink < Minitest::Test
  def test_write_tile_wrong_buffer_size
    Dir.mktmpdir do |tmp|
      path = File.join(tmp, 'bad.tif')
      sink = Ptiff::ptiff_sink_create(path, tile_descriptor)
      refute_nil sink, 'sink_create returned NULL'
      begin
        bs = Ptiff::ptiff_sink_tile_byte_size(sink)
        refute_equal 0, Ptiff::ptiff_sink_write_tile(sink, 0, 0, "\0".b * (bs - 1))
        refute_equal 0, Ptiff::ptiff_sink_write_tile(sink, 0, 0, "\0".b * (bs + 1))
      ensure
        Ptiff::ptiff_sink_close(sink)
      end
    end
  end

  def test_write_tile_out_of_range
    Dir.mktmpdir do |tmp|
      path = File.join(tmp, 'oor.tif')
      sink = Ptiff::ptiff_sink_create(path, tile_descriptor)
      refute_nil sink, 'sink_create returned NULL'
      begin
        buf = "\0".b * Ptiff::ptiff_sink_tile_byte_size(sink)
        refute_equal 0, Ptiff::ptiff_sink_write_tile(sink, 99, 0, buf)
        refute_equal 0, Ptiff::ptiff_sink_write_tile(sink, 0, 99, buf)
      ensure
        Ptiff::ptiff_sink_close(sink)
      end
    end
  end

  def test_create_sink_empty_path
    sink = Ptiff::ptiff_sink_create('', tile_descriptor)
    unless sink.nil?
      Ptiff::ptiff_sink_close(sink)
      flunk 'sink_create("") expected nil'
    end
  end

  def test_create_sink_untiled_descriptor
    desc = Ptiff::Ptiff_image_descriptor.new
    desc.width = 16
    desc.height = 16
    desc.pixel_type = Ptiff::PTIFF_PIXEL_UINT8
    desc.channel_count = 1

    Dir.mktmpdir do |tmp|
      path = File.join(tmp, 'untiled.tif')
      sink = Ptiff::ptiff_sink_create(path, desc)
      unless sink.nil?
        Ptiff::ptiff_sink_close(sink)
        flunk 'sink_create on an untiled descriptor expected nil'
      end
      assert !File.exist?(path), 'untiled sink_create must not leave a file behind'
    end
  end
end
