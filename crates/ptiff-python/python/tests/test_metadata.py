"""Read-only metadata + generic extension-field (PDS-layer) parity."""

import numpy as np
import ptiff_pyo3 as ptiff


def test_metadata_descriptor(tmp_path):
    path = str(tmp_path / "md.tif")
    sink = ptiff.create_image(
        path,
        width=128,
        height=64,
        pixel_type=ptiff.PTIFF_PIXEL_UINT16,
        channel_count=3,
        tile_width=64,
        tile_height=32,
    )
    data = np.arange(64 * 32 * 3, dtype=np.uint16) % 1000
    sink.write_tile(0, 0, data.tobytes())
    sink.write_tile(1, 0, data.tobytes())
    sink.write_tile(0, 1, data.tobytes())
    sink.write_tile(1, 1, data.tobytes())
    sink.close()

    md = ptiff.open(path).metadata()
    assert md.width == 128
    assert md.height == 64
    assert md.channel_count == 3
    assert md.pixel_type_name == "uint16"
    assert md.pixel_type == ptiff.PTIFF_PIXEL_UINT16
    assert md.tile_width == 64
    assert md.tile_height == 32
    # No generic ptiff.* fields written in this simple image.
    assert dict(md.fields) == {}


def test_metadata_field_lookup(tmp_path):
    path = str(tmp_path / "mdf.tif")
    sink = ptiff.create_image(path, width=64, height=32)
    sink.write_tile(0, 0, bytes(sink.tile_byte_size))
    sink.close()
    md = ptiff.open(path).metadata()
    assert md.field("ptiff.spice.frame") is None
    assert md.field("ptiff.spice.frame", "unset") == "unset"
