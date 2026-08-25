"""pytest roots configured here so the ``ptiff_mcp`` package is importable and
the reference TIFF is generated on demand with ``ptiff_pyo3`` (the PyO3
binding), so the MCP tests need no pre-existing/foreign files.
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_MCP = os.path.dirname(_HERE)

# Expose the MCP package on sys.path for the whole session.
_ADD = [os.path.join(_MCP, "src")]
for p in _ADD:
    if p not in sys.path:
        sys.path.insert(0, p)

import pytest  # noqa: E402
import ptiff_pyo3 as _ptiff  # noqa: E402


@pytest.fixture(scope="session")
def sample_tiff(tmp_path_factory):
    """A small 32x32 uint8, 16x16-tile reference PTIFF built by ``ptiff_pyo3``.

    Generated (not checked in) so the MCP tests run without foreign files:
    the old SWIG ``bindings/python/test/roundtrip_python.tif`` is gone.
    """
    path = tmp_path_factory.mktemp("data") / "reference.tif"
    sink = _ptiff.create_image(
        str(path),
        32,
        32,
        pixel_type=_ptiff.PTIFF_PIXEL_UINT8,
        channel_count=1,
        tile_width=16,
        tile_height=16,
        compression=_ptiff.PTIFF_COMPRESSION_NONE,
    )
    tile_bytes = sink.tile_byte_size
    # Fill every tile with a repeating 0..255 ramp so per-tile stats are
    # well-defined (min 0, max 255 in each tile).
    ramp = bytes(range(256))
    payload = (ramp * ((tile_bytes // 256) + 1))[:tile_bytes]
    for row in range(sink.tile_rows):
        for col in range(sink.tile_columns):
            sink.write_tile(col, row, payload)
    sink.close()
    return str(path)
