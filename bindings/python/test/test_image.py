"""Image descriptor surface over the raw SWIG-generated Python binding.

Ports bindings/python/test/test_ptiff.py's test_image_roundtrip and
test_image_optionals_absent onto ptiff_image_descriptor/ptiff_image_create.
Requires the double*/int* OUTPUT typemaps added to typemaps.i for
ptiff_image_gsd/ptiff_image_compression (ptiff_image_tile_info already worked
without one: its out-param is a SWIG-wrapped struct pointer, not a bare
primitive).
"""

import unittest

import ptiff


class TestImage(unittest.TestCase):
    def test_image_roundtrip(self) -> None:
        desc = ptiff.ptiff_image_descriptor()
        desc.width = 10
        desc.height = 20
        desc.pixel_type = ptiff.PTIFF_PIXEL_UINT16
        desc.channel_count = 3
        desc.has_gsd = 1
        desc.gsd = 0.5
        desc.has_tile_info = 1
        desc.tile_info.tile_width = 16
        desc.tile_info.tile_height = 16
        desc.has_compression = 1
        desc.compression = ptiff.PTIFF_COMPRESSION_DEFLATE

        img = ptiff.ptiff_image_create(desc)
        self.assertTrue(img, "image_create returned NULL")
        self.addCleanup(ptiff.ptiff_image_destroy, img)

        self.assertEqual(ptiff.ptiff_image_width(img), 10)
        self.assertEqual(ptiff.ptiff_image_height(img), 20)
        self.assertEqual(ptiff.ptiff_image_pixel_type(img), ptiff.PTIFF_PIXEL_UINT16)
        self.assertEqual(ptiff.ptiff_image_channel_count(img), 3)

        ok, gsd = ptiff.ptiff_image_gsd(img)
        self.assertTrue(ok)
        self.assertEqual(gsd, 0.5)

        ti = ptiff.ptiff_tile_info()
        ok = ptiff.ptiff_image_tile_info(img, ti)
        self.assertTrue(ok)
        self.assertEqual((ti.tile_width, ti.tile_height), (16, 16))

        ok, comp = ptiff.ptiff_image_compression(img)
        self.assertTrue(ok)
        self.assertEqual(comp, ptiff.PTIFF_COMPRESSION_DEFLATE)

    def test_image_optionals_absent(self) -> None:
        desc = ptiff.ptiff_image_descriptor()
        desc.width = 4
        desc.height = 4
        desc.pixel_type = ptiff.PTIFF_PIXEL_FLOAT32
        desc.channel_count = 1

        img = ptiff.ptiff_image_create(desc)
        self.assertTrue(img, "image_create returned NULL")
        self.addCleanup(ptiff.ptiff_image_destroy, img)

        ok, _ = ptiff.ptiff_image_gsd(img)
        self.assertFalse(ok)

        ti = ptiff.ptiff_tile_info()
        ok = ptiff.ptiff_image_tile_info(img, ti)
        self.assertFalse(ok)

        ok, _ = ptiff.ptiff_image_compression(img)
        self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
