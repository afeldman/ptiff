#!/usr/bin/env python3
"""Benchmark read/write throughput of the PyO3 Python binding (ptiff_pyo3).

Measures, with median-of-repeats timing (perf_counter — same methodology as
scripts/interop_check.py):

  * write_all_tiles   -- create a 128x128 UInt8 tiled TIFF and write every tile
                         through the Sink surface (4 tiles of 64x64).
  * read_uint8_128    -- open benchmarks/fixtures/uint8_128.tif, read all tiles.
  * read_uint8_512    -- open benchmarks/fixtures/uint8_512.tif, read all tiles.
  * read_f32_512      -- open benchmarks/fixtures/f32_512.tif, read all tiles.

Writes a per-language JSON document to --out (consumed by run_benchmarks.sh /
summary.py). All repeat/time/iteration knobs are env- or argument-driven so runs
are reproducible and consistent across the whole suite.

The Python binding is the PyO3 module `ptiff_pyo3` (crates/ptiff-python, built
with maturin) now imported here as `ptiff`; the legacy SWIG `ptiff` package and
its C-ABI functions (ptiff_sink_*, ptiff_source_*) have been removed.

Usage:
    python3 benchmarks/src/bench_python.py \
        --out benchmarks/benchmark-results/python.json
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import time
from pathlib import Path

import ptiff_pyo3 as ptiff

HERE = Path(__file__).resolve().parent
FIXTURES = HERE.parent / "fixtures"
REPEATS = int(os.environ.get("BENCH_REPEATS", "20"))
# Inner iterations per timed sample, so each timed run loops the tiled
# read/write of the image body `ITERS` times (noise amortization). Larger
# ITERS = stabler median, longer wall time.
ITERS = int(os.environ.get("BENCH_ITERS", "50"))
# Inner iterations for the heavy NAC metric (616 tiles per pass); a small
# default keeps the ~55 MB real-NAC read cheap even at default BENCH_REPEATS.
NAC_ITERS = max(1, int(os.environ.get("BENCH_NAC_ITERS", "2")))

WRITE_SIZE = 128
WRITE_TILE = 64


def median_times(fn, repeats: int) -> dict:
    times = []
    for _ in range(repeats):
        t0 = time.perf_counter()
        fn()
        times.append((time.perf_counter() - t0) * 1000.0)
    times.sort()
    return {
        "repeats": repeats,
        "median_ms": round(times[len(times) // 2], 4),
        "min_ms": round(times[0], 4),
        "max_ms": round(times[-1], 4),
    }


def write_all_tiles(tmpdir: Path, iters: int) -> None:
    out = tmpdir / "write_small.tif"
    if out.exists():
        out.unlink()
    sink = ptiff.create_image(
        str(out),
        width=WRITE_SIZE,
        height=WRITE_SIZE,
        pixel_type=0,
        channel_count=1,
        tile_width=WRITE_TILE,
        tile_height=WRITE_TILE,
        compression=0,
    )
    cols = sink.tile_columns
    rows = sink.tile_rows
    bs = sink.tile_byte_size
    pat = bytes([7]) * bs
    for _ in range(iters):
        for c in range(cols):
            for r in range(rows):
                sink.write_tile(c, r, pat)
    sink.close()


def read_all_tiles(path: Path, iters: int) -> None:
    doc = ptiff.open(str(path))
    img = doc.image(0)
    ncols = img.tile_columns
    nrows = img.tile_rows
    for _ in range(iters):
        for c in range(ncols):
            for r in range(nrows):
                # read_tile returns a fresh numpy.ndarray; discarded here
                # (we measure raw tile-read throughput).
                img.read_tile(column=c, row=r)
    # The Document is released on garbage collection (no explicit close);
    # dropping the references here frees the parser before the next sample.
    del doc


def fixture_info(name: str) -> dict:
    p = FIXTURES / name
    return {
        "file": str(p),
        "size_bytes": p.stat().st_size,
        "exists": p.exists(),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument(
        "--real",
        action="store_true",
        help="also read fixtures/real_lola_512.tif (a real NASA LOLA "
        "elevation crop) if present",
    )
    ap.add_argument(
        "--nac",
        action="store_true",
        help="also read fixtures/nac_dtm.tif (a real NASA LRO-NAC "
        "DTM, 2693x14236) if present",
    )
    args = ap.parse_args()
    real = args.real
    nac = args.nac

    # Warm-up: load the module + libptiff backends once before any measurements.
    # Use a minimal but valid tiled image (tiles must be nonzero multiples of 16).
    ptiff.create_image(
        str(Path(tempfile.gettempdir()) / "_ptiff_warmup.tif"),
        width=16,
        height=16,
        pixel_type=0,
        channel_count=1,
        tile_width=16,
        tile_height=16,
        compression=0,
    ).close()

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        # quick self-check that writes are valid (do once, cheap)
        write_all_tiles(tmp, 1)

        metrics = {
            "write_all_tiles_ms": median_times(
                lambda: write_all_tiles(tmp, ITERS), REPEATS
            ),
            "read_uint8_128_ms": median_times(
                lambda: read_all_tiles(FIXTURES / "uint8_128.tif", ITERS), REPEATS
            ),
            "read_uint8_512_ms": median_times(
                lambda: read_all_tiles(FIXTURES / "uint8_512.tif", ITERS), REPEATS
            ),
            "read_f32_512_ms": median_times(
                lambda: read_all_tiles(FIXTURES / "f32_512.tif", ITERS), REPEATS
            ),
        }

    major, minor, patch = ptiff.runtime_version()
    doc = {
        "language": "python",
        "binding_version": f"{major}.{minor}.{patch}",
        "repeats": REPEATS,
        "write_image": {
            "width": WRITE_SIZE,
            "height": WRITE_SIZE,
            "pixel_type": "uint8",
            "tile": WRITE_TILE,
        },
        "iterations_per_sample": ITERS,
        "fixtures": {
            "uint8_128": fixture_info("uint8_128.tif"),
            "uint8_512": fixture_info("uint8_512.tif"),
            "f32_512": fixture_info("f32_512.tif"),
        },
        "metrics": metrics,
    }

    # --real: read a genuine NASA LOLA elevation crop (copied into benchmarks/
    # fixtures by run_benchmarks.sh --real) as an extra data point on real
    # scientific data in addition to the synthetic gradient fixtures.
    if real:
        real_tif = FIXTURES / "real_lola_512.tif"
        if real_tif.exists():
            metrics["read_real_lola_512_ms"] = median_times(
                lambda: read_all_tiles(real_tif, ITERS), REPEATS
            )
            doc["fixtures"]["real_lola_512"] = fixture_info("real_lola_512.tif")
        else:
            print(
                "[python] --real requested but real_lola_512.tif missing; skipping",
                file=sys.stderr,
            )

    # --nac: read the full, real NASA LRO-NAC DTM (616 tiles/pass, ~55 MB,
    # copied into benchmarks/fixtures by run_benchmarks.sh --nac). Each inner
    # sample is a full pass over every tile; NAC_ITERS keeps it bounded.
    if nac:
        nac_tif = FIXTURES / "nac_dtm.tif"
        if nac_tif.exists():
            metrics["read_nac_ms"] = median_times(
                lambda: read_all_tiles(nac_tif, NAC_ITERS), REPEATS
            )
            doc["fixtures"]["nac_dtm"] = fixture_info("nac_dtm.tif")
            doc["nac_ifd"] = {"tiles_per_pass": 616, "nac_iters": NAC_ITERS}
        else:
            print(
                "[python] --nac requested but nac_dtm.tif missing; skipping",
                file=sys.stderr,
            )

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, indent=2) + "\n")
    print(json.dumps({"language": "python", "metrics": metrics}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
