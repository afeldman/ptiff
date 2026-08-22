# frozen_string_literal: true

# Error-code / pixel-type / compression-kind numeric ordering pins.
#
# There is no PTiff::Error/PixelType/CompressionKind enum wrapper at the raw
# SWIG level (that is hand-written-binding sugar): what the C ABI actually
# promises is the numeric ordering of ptiff_error_code / ptiff_pixel_type /
# ptiff_compression_kind, which must match the C++ enums -- this pins that
# ordering so a future re-numbering is caught here. Mirrors
# bindings/swig/go/error_test.go and bindings/swig/python/test_error.py.
require 'minitest/autorun'
require 'ptiff'

class TestErrorCodeOrdering < Minitest::Test
  def test_ordering
    {
      Ptiff::PTIFF_ERROR_NOT_IMPLEMENTED => 0,
      Ptiff::PTIFF_ERROR_INVALID_ARGUMENT => 1,
      Ptiff::PTIFF_ERROR_OUT_OF_RANGE => 2,
      Ptiff::PTIFF_ERROR_NOT_FOUND => 3,
      Ptiff::PTIFF_ERROR_UNKNOWN => 4,
    }.each { |code, want| assert_equal want, code }
  end
end

class TestPixelTypeOrdering < Minitest::Test
  def test_ordering
    {
      Ptiff::PTIFF_PIXEL_UINT8 => 0,
      Ptiff::PTIFF_PIXEL_UINT16 => 1,
      Ptiff::PTIFF_PIXEL_UINT32 => 2,
      Ptiff::PTIFF_PIXEL_FLOAT32 => 3,
      Ptiff::PTIFF_PIXEL_FLOAT64 => 4,
    }.each { |pt, want| assert_equal want, pt }
  end
end

class TestCompressionKindOrdering < Minitest::Test
  def test_ordering
    {
      Ptiff::PTIFF_COMPRESSION_NONE => 0,
      Ptiff::PTIFF_COMPRESSION_LZW => 1,
      Ptiff::PTIFF_COMPRESSION_DEFLATE => 2,
      Ptiff::PTIFF_COMPRESSION_JPEG => 3,
    }.each { |c, want| assert_equal want, c }
  end
end
