"""Full write -> read pixel roundtrip (feature parity with SWIG sink/source).

Writes a tiled TIFF through `create_image`/`Sink.write_tile`/`Sink.close`,
then reads it back through `open`/`Image.read_tile`.
"""

import numpy as np
import ptiff_pyo3 as ptiff


def _write_tiled(path, width=64, height=32, tile=16, code=0, channels=1):
    sink = ptiff.create_image(
        path,
        width=width,
        height=height,
        pixel_type=code,
        channel_count=channels,
        tile_width=tile,
        tile_height=tile,
    )
    cols = sink.tile_columns
    rows = sink.tile_rows
    bs = sink.tile_byte_size
    for c in range(cols):
        for r in range(rows):
            pattern = (np.arange(bs, dtype=np.uint8) + c * rows + r) % 251
            sink.write_tile(c, r, pattern.tobytes())
    sink.close()
    return cols, rows, bs


def test_write_read_roundtrip(tmp_path):
    path = str(tmp_path / "rt.tif")
    cols, rows, bs = _write_tiled(path)
    assert (cols, rows) == (4, 2)  # 64/16 x 32/16

    doc = ptiff.open(path)
    assert doc.image_count() == 1
    img = doc.image(0)
    assert img.width == 64
    assert img.height == 32
    assert img.pixel_type == "uint8"
    assert img.channel_count == 1
    assert img.tile_width == 16
    assert img.tile_height == 16
    assert img.tile_columns == cols
    assert img.tile_rows == rows

    # Verify tile (2,1) read back exactly.
    tile = img.read_tile(column=2, row=1)
    assert isinstance(tile, np.ndarray)
    assert tile.shape == (16, 16, 1)
    assert tile.dtype == np.uint8
    expected = (np.arange(bs, dtype=np.uint8) + 2 * rows + 1) % 251
    assert (tile.ravel() == expected).all()


def test_uint16_roundtrip(tmp_path):
    path = str(tmp_path / "rt16.tif")
    sink = ptiff.create_image(
        path,
        width=64,
        height=32,
        pixel_type=ptiff.PTIFF_PIXEL_UINT16,
        channel_count=1,
        tile_width=64,
        tile_height=32,
    )
    data = np.arange(64 * 32, dtype=np.uint16) % 7000
    sink.write_tile(0, 0, data.tobytes())
    sink.close()

    img = ptiff.open(path).image(0)
    tile = img.read_tile(column=0, row=0)
    assert tile.dtype == np.uint16
    assert tile.shape == (32, 64, 1)
    assert (tile.ravel().astype(np.uint16) == data).all()


def test_sink_write_tile_checks_byte_size(tmp_path):
    path = str(tmp_path / "bad.tif")
    sink = ptiff.create_image(path, width=64, height=32, tile_width=16, tile_height=16)
    bs = sink.tile_byte_size
    import pytest

    with pytest.raises(ValueError):
        sink.write_tile(0, 0, bytes(bs - 1))


def test_sink_index_out_of_range(tmp_path):
    path = str(tmp_path / "oor.tif")
    sink = ptiff.create_image(path, width=64, height=32, tile_width=16, tile_height=16)
    import pytest

    buf = bytes(sink.tile_byte_size)
    with pytest.raises(ValueError):
        sink.write_tile(99, 0, buf)
    with pytest.raises(ValueError):
        sink.write_tile(0, 99, buf)


def test_document_is_context_manager(tmp_path):
    path = str(tmp_path / "ctx.tif")
    _write_tiled(path)
    with ptiff.open(path) as doc:
        assert doc.image_count() == 1
        assert doc.image(0).height == 32
