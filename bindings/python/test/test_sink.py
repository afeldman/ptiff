"""ptiff_sink error-path coverage over the raw SWIG-generated Python binding,
complementing the happy-path round trip in test_roundtrip.py.

Ports the safe subset of bindings/python's sink coverage (mirrors
bindings/swig/go/sink_test.go). Not ported: double-close / write-after-close
-- the raw SWIG handle has no close-once guard (that is hand-written-binding
sugar), so a second ptiff_sink_close is a C-level double-free and writing
through an already-closed pointer is a use-after-free. Neither is safe to
exercise against the real C ABI.

Unlike the Go binding, a NULL ptiff_sink* surfaces as plain Python None here
(SWIG's default opaque-pointer typemap for Python), so `sink is None` is a
correct, no-typemap-needed NULL check -- no Swigcptr()-style workaround.
"""

import os
import tempfile
import unittest

import ptiff


def _tile_descriptor() -> ptiff.ptiff_image_descriptor:
    desc = ptiff.ptiff_image_descriptor()
    desc.width = 32
    desc.height = 32
    desc.pixel_type = ptiff.PTIFF_PIXEL_UINT8
    desc.channel_count = 1
    desc.has_tile_info = 1
    desc.tile_info.tile_width = 16
    desc.tile_info.tile_height = 16
    return desc


class TestSink(unittest.TestCase):
    def test_write_tile_wrong_buffer_size(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "bad.tif")
            sink = ptiff.ptiff_sink_create(path, _tile_descriptor())
            self.assertIsNotNone(sink, "sink_create returned NULL")
            self.addCleanup(ptiff.ptiff_sink_close, sink)

            bs = ptiff.ptiff_sink_tile_byte_size(sink)
            rc_short = ptiff.ptiff_sink_write_tile(sink, 0, 0, bytes(bs - 1))
            self.assertNotEqual(rc_short, 0)
            rc_long = ptiff.ptiff_sink_write_tile(sink, 0, 0, bytes(bs + 1))
            self.assertNotEqual(rc_long, 0)

    def test_write_tile_out_of_range(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "oor.tif")
            sink = ptiff.ptiff_sink_create(path, _tile_descriptor())
            self.assertIsNotNone(sink, "sink_create returned NULL")
            self.addCleanup(ptiff.ptiff_sink_close, sink)

            buf = bytes(ptiff.ptiff_sink_tile_byte_size(sink))
            self.assertNotEqual(ptiff.ptiff_sink_write_tile(sink, 99, 0, buf), 0)
            self.assertNotEqual(ptiff.ptiff_sink_write_tile(sink, 0, 99, buf), 0)

    def test_create_sink_empty_path(self) -> None:
        sink = ptiff.ptiff_sink_create("", _tile_descriptor())
        if sink is not None:
            ptiff.ptiff_sink_close(sink)
            self.fail('sink_create("") expected None')

    def test_create_sink_untiled_descriptor(self) -> None:
        desc = ptiff.ptiff_image_descriptor()
        desc.width = 16
        desc.height = 16
        desc.pixel_type = ptiff.PTIFF_PIXEL_UINT8
        desc.channel_count = 1

        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "untiled.tif")
            sink = ptiff.ptiff_sink_create(path, desc)
            if sink is not None:
                ptiff.ptiff_sink_close(sink)
                self.fail("sink_create on an untiled descriptor expected None")
            self.assertFalse(
                os.path.exists(path), "untiled sink_create must not leave a file behind"
            )


if __name__ == "__main__":
    unittest.main()
