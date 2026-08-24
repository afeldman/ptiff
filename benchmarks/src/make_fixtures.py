#!/usr/bin/env python3
"""Generate the reproducible tiled TIFF fixtures used by all language benchmarks.

The benchmark suite wants a shared, deterministic set of real on-disk TIFFs so
that read throughput across the language bindings is measured against *identical*
byte streams (and write throughput against identical logical images). Rather than
depend on gitignored, externally-downloaded samples (scripts/samples/), we write
the fixtures ourselves through the same SWIG C-ABI binding the benchmarks measure
-- which has the pleasant side effect that fixture generation itself is a small,
validated exercise of the sink/source surface.

Fixtures written into benchmarks/fixtures/ (gitignored):

    uint8_128.tif    128x128  UInt8    tile 64x64   -> 4 tiles   (tiny, smoke)
    uint8_512.tif    512x512  UInt8    tile 128x128 -> 16 tiles
    f32_512.tif      512x512  Float32  tile 128x128 -> 16 tiles

Pixel values are deterministic: UInt8 uses the same gradient formula
`(x*3 + y*5) % 256` (as used by the historical C++ `gen_interop_fixture.cpp`,
removed with libptiff) so it is reproducible and matches the interop fixtures;
Float32 uses a normalized ramp in [0,1].

Requires the Python binding on PYTHONPATH (bindings/python/src) -- build it with
`make -C bindings/swig python` first.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

import ptiff

FIXTURES_DIR = Path(__file__).resolve().parent.parent / "fixtures"

PIXEL_BYTES = {0: 1, 1: 2, 2: 4, 3: 4, 4: 8}


def _write(path: Path, width: int, height: int, pixel_type: int, tile: int) -> int:
    byte_width = PIXEL_BYTES.get(pixel_type)
    if byte_width is None:
        raise ValueError(f"unhandled pixel_type {pixel_type}")

    if path.exists():
        path.unlink()
    d = ptiff.ptiff_image_descriptor()
    d.width = width
    d.height = height
    d.pixel_type = pixel_type
    d.channel_count = 1
    d.has_tile_info = 1
    d.tile_info.tile_width = tile
    d.tile_info.tile_height = tile
    d.has_compression = 0

    sink = ptiff.ptiff_sink_create(str(path), d)
    if not sink:
        sys.exit(f"sink_create failed for {path}")
    cols = ptiff.ptiff_sink_tile_columns(sink)
    rows = ptiff.ptiff_sink_tile_rows(sink)
    bs = ptiff.ptiff_sink_tile_byte_size(sink)

    for c in range(cols):
        for r in range(rows):
            if pixel_type == 0:  # UInt8 gradient (matches the interop fixtures)
                pattern = bytes([(3 * (c * tile) + 5 * (r * tile)) % 256]) * bs
            elif pixel_type == 3:  # Float32 ramp in [0, 1]
                base = (c * cols + r) / float(cols * rows)
                pattern = b"".join(
                    struct.pack("<f", base + (i % 7) * 0.01) for i in range(bs // 4)
                )
            else:  # UInt16 packed ramp
                base = (c * cols + r) * 257
                pattern = b"".join(
                    struct.pack("<H", (base + i) & 0xFFFF) for i in range(bs // 2)
                )

            rc = ptiff.ptiff_sink_write_tile(sink, c, r, pattern)
            if rc != 0:
                sys.exit(f"write_tile({c},{r}) rc={rc}")
    ptiff.ptiff_sink_close(sink)
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
