# frozen_string_literal: true

# Image descriptor surface over the raw SWIG-generated Ruby binding.
#
# Ports bindings/ruby/test/test_ptiff.rb's test_image_roundtrip and
# test_image_optionals_absent onto Ptiff_image_descriptor/ptiff_image_create.
# Requires the double*/int* OUTPUT typemaps added to typemaps.i for
# ptiff_image_gsd/ptiff_image_compression (ptiff_image_tile_info already
# worked without one: its out-param is a SWIG-wrapped struct pointer, not a
# bare primitive).
require 'minitest/autorun'
require 'ptiff'

class TestImage < Minitest::Test
  def test_image_roundtrip
    desc = Ptiff::Ptiff_image_descriptor.new
    desc.width = 10
    desc.height = 20
    desc.pixel_type = Ptiff::PTIFF_PIXEL_UINT16
    desc.channel_count = 3
    desc.has_gsd = 1
    desc.gsd = 0.5
    desc.has_tile_info = 1
    desc.tile_info.tile_width = 16
    desc.tile_info.tile_height = 16
    desc.has_compression = 1
    desc.compression = Ptiff::PTIFF_COMPRESSION_DEFLATE

    img = Ptiff::ptiff_image_create(desc)
    refute_nil img, 'image_create returned NULL'
    begin
      assert_equal 10, Ptiff::ptiff_image_width(img)
      assert_equal 20, Ptiff::ptiff_image_height(img)
      assert_equal Ptiff::PTIFF_PIXEL_UINT16, Ptiff::ptiff_image_pixel_type(img)
      assert_equal 3, Ptiff::ptiff_image_channel_count(img)

      ok, gsd = Ptiff::ptiff_image_gsd(img)
      assert ok != 0
      assert_in_delta 0.5, gsd, 1e-9

      ti = Ptiff::Ptiff_tile_info.new
      ok = Ptiff::ptiff_image_tile_info(img, ti)
      assert ok != 0
      assert_equal [16, 16], [ti.tile_width, ti.tile_height]

      ok, comp = Ptiff::ptiff_image_compression(img)
      assert ok != 0
      assert_equal Ptiff::PTIFF_COMPRESSION_DEFLATE, comp
    ensure
      Ptiff::ptiff_image_destroy(img)
    end
  end

  def test_image_optionals_absent
    desc = Ptiff::Ptiff_image_descriptor.new
    desc.width = 4
    desc.height = 4
    desc.pixel_type = Ptiff::PTIFF_PIXEL_FLOAT32
    desc.channel_count = 1

    img = Ptiff::ptiff_image_create(desc)
    refute_nil img, 'image_create returned NULL'
    begin
      ok, = Ptiff::ptiff_image_gsd(img)
      assert_equal 0, ok

      ti = Ptiff::Ptiff_tile_info.new
      ok = Ptiff::ptiff_image_tile_info(img, ti)
      assert_equal 0, ok

      ok, = Ptiff::ptiff_image_compression(img)
      assert_equal 0, ok
    ensure
      Ptiff::ptiff_image_destroy(img)
    end
  end
end
