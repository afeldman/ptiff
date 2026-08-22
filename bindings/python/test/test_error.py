"""Error-code / pixel-type / compression-kind numeric ordering pins.

There is no Error/PixelType/CompressionKind enum wrapper at the raw SWIG
level (that is hand-written-binding sugar): what the C ABI actually promises
is the numeric ordering of ptiff_error_code / ptiff_pixel_type /
ptiff_compression_kind, which must match the C++ enums -- this pins that
ordering so a future re-numbering is caught here. Mirrors
bindings/swig/go/error_test.go and the ordering tests in
bindings/swig/go/image_test.go.
"""

import unittest

import ptiff


class TestErrorCodeOrdering(unittest.TestCase):
    def test_ordering(self) -> None:
        cases = [
            (ptiff.PTIFF_ERROR_NOT_IMPLEMENTED, 0),
            (ptiff.PTIFF_ERROR_INVALID_ARGUMENT, 1),
            (ptiff.PTIFF_ERROR_OUT_OF_RANGE, 2),
            (ptiff.PTIFF_ERROR_NOT_FOUND, 3),
            (ptiff.PTIFF_ERROR_UNKNOWN, 4),
        ]
        for code, want in cases:
            self.assertEqual(code, want)


class TestPixelTypeOrdering(unittest.TestCase):
    def test_ordering(self) -> None:
        cases = [
            (ptiff.PTIFF_PIXEL_UINT8, 0),
            (ptiff.PTIFF_PIXEL_UINT16, 1),
            (ptiff.PTIFF_PIXEL_UINT32, 2),
            (ptiff.PTIFF_PIXEL_FLOAT32, 3),
            (ptiff.PTIFF_PIXEL_FLOAT64, 4),
        ]
        for pt, want in cases:
            self.assertEqual(pt, want)


class TestCompressionKindOrdering(unittest.TestCase):
    def test_ordering(self) -> None:
        cases = [
            (ptiff.PTIFF_COMPRESSION_NONE, 0),
            (ptiff.PTIFF_COMPRESSION_LZW, 1),
            (ptiff.PTIFF_COMPRESSION_DEFLATE, 2),
            (ptiff.PTIFF_COMPRESSION_JPEG, 3),
        ]
        for c, want in cases:
            self.assertEqual(c, want)


if __name__ == "__main__":
    unittest.main()
