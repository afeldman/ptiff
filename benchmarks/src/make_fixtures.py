#!/usr/bin/env python3
"""Generate the reproducible tiled TIFF fixtures used by all language benchmarks.

The benchmark suite wants a shared, deterministic set of real on-disk TIFFs so
that read throughput across the language bindings is measured against *identical*
byte streams (and write throughput against identical logical images). Rather than
depend on gitignored, externally-downloaded samples (scripts/samples/), we write
the fixtures ourselves through the same PyO3 Python binding the benchmarks
measure -- which has the pleasant side effect that fixture generation itself is a
small, validated exercise of the create_image/write_tile/close surface.

The Python binding is the PyO3 module `ptiff_pyo3` in crates/ptiff-python
(built with maturin; the SWIG C-ABI `bindings/python` package has been removed).
It is imported here as `ptiff` for readability.

Fixtures written into benchmarks/fixtures/ (gitignored):

    uint8_128.tif    128x128  UInt8    tile 64x64   -> 4 tiles   (tiny, smoke)
    uint8_512.tif    512x512  UInt8    tile 128x128 -> 16 tiles
    f32_512.tif      512x512  Float32  tile 128x128 -> 16 tiles

Pixel values are deterministic: UInt8 uses the same gradient formula
`(x*3 + y*5) % 256` (as used by the historical C++ `gen_interop_fixture.cpp`,
removed with libptiff) so it is reproducible and matches the interop fixtures;
Float32 uses a normalized ramp in [0,1].

Requires the Python binding installed (crates/ptiff-python; `maturin develop`
inside a venv). Import as `ptiff_pyo3` or `ptiff` (same extension).
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np
import ptiff_pyo3 as ptiff

FIXTURES_DIR = Path(__file__).resolve().parent.parent / "fixtures"


def _write(path: Path, width: int, height: int, pixel_type: int, tile: int) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()

    sink = ptiff.create_image(
        str(path),
        width=width,
        height=height,
        pixel_type=pixel_type,
        channel_count=1,
        tile_width=tile,
        tile_height=tile,
        compression=0,
    )
    cols = sink.tile_columns
    rows = sink.tile_rows
    bs = sink.tile_byte_size

    for c in range(cols):
        for r in range(rows):
            if pixel_type == 0:  # UInt8 gradient (matches the interop fixtures)
                pattern = bytes([(3 * c * tile + 5 * r * tile) % 256]) * bs
            elif pixel_type == 3:  # Float32 ramp in [0, 1]
                base = (c * cols + r) / float(cols * rows)
                pattern = np.ones(bs // 4, dtype=np.float32) * base
                # Slight deterministic variation per sample in the tile.
                pattern += (np.arange(bs // 4, dtype=np.float32) % 7) * 0.01
                pattern = pattern.tobytes()
            else:  # UInt16 packed ramp
                base = (c * cols + r) * 257
                data = (base + np.arange(bs // 2, dtype=np.uint16)) & 0xFFFF
                pattern = data.tobytes()

            sink.write_tile(c, r, pattern)

    sink.close()
    return path.stat().st_size


def main() -> int:
    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    specs = [
        ("uint8_128.tif", 128, 128, 0, 64),
        ("uint8_512.tif", 512, 512, 0, 128),
        ("f32_512.tif", 512, 512, 3, 128),
    ]
    for name, w, h, pt, tile in specs:
        path = FIXTURES_DIR / name
        n = _write(path, w, h, pt, tile)
        print(f"{name}: {w}x{h} pixel_type={pt} tile={tile} -> {n} bytes")
    print(f"fixtures in {FIXTURES_DIR}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
