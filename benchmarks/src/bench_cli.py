#!/usr/bin/env python3
"""Benchmark the ptiff CLI end-to-end as a fresh subprocess each sample.

The per-language scripts measure *in-process* tile throughput; this one
measures the whole process lifetime of the Rust ptiff CLI (`ptiff info <file>`):
process spawn, C-ABI dylib load, TIFF open/IFD parse, printing and exit. That is
the real cost a user pays per CLI invocation.

Each sample is one full subprocess run (start -> exit). Because the unit being
timed *is* the subprocess itself, there is no inner iteration (BENCH_ITERS does
not apply) -- only an outer median over BENCH_REPEATS samples, matching the
median-of-repeats methodology of the rest of the suite.

Metrics:
    cli_info_uint8_128_ms   `ptiff info fixtures/uint8_128.tif` (tiny file;
                            dominated by process/library load + parse)
    cli_info_nac_ms         `ptiff info scripts/samples/NAC_DTM_ATLAS2.PYR.TIF`
                            (real ~55 MB NASA LRO-NAC pyramid; large IFD parse)
                            omitted if the sample is not cached locally
    cli_copy_nac_ms         `ptiff copy` of the real LRO-NAC DTM (256x256 tiles);
                            full end-to-end pixel read+write through the CLI
                            (fresh subprocess, throwaway output); skipped if the
                            NAC sample is not cached locally

Output JSON (--out):
    {
        "language": "cli",
        "binary": "<abs path to ptiff>",
        "repeats": N,
        "metrics": { "<name>": {repeats, median_ms, min_ms, max_ms}, ... }
    }

Usage:
    DYLD_LIBRARY_PATH=install-shared/lib \
        python3 src/bench_cli.py --bin /abs/ptiff --out benchmark-results/cli.json
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent  # benchmarks/
PROJECT = REPO.parent  # repo root
FIXTURES = REPO / "fixtures"
SAMPLES = PROJECT / "scripts" / "samples"


def _run_once(bin_path: str, file: Path) -> float:
    t0 = time.perf_counter()
    subprocess.run([bin_path, "info", str(file)], capture_output=True, check=True)
    return (time.perf_counter() - t0) * 1000.0


def _run_copy_once(bin_path: str, file: Path, dst: Path) -> float:
    # Full end-to-end pixel copy: fresh subprocess opens `file` via the C ABI
    # read side and writes all tiles to `dst`. The output is discarded after.
    t0 = time.perf_counter()
    subprocess.run(
        [bin_path, "copy", str(file), str(dst)], capture_output=True, check=True
    )
    return (time.perf_counter() - t0) * 1000.0


def median_samples(repeats: int, fn) -> dict:
    times: list[float] = sorted(fn() for _ in range(repeats))
    return {
        "repeats": repeats,
        "median_ms": round(statistics.median(times), 4),
        "min_ms": round(times[0], 4),
        "max_ms": round(times[-1], 4),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--bin", required=True, help="absolute path to the ptiff CLI binary"
    )
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    bin_path = str(Path(args.bin).expanduser().resolve())
    if not Path(bin_path).exists():
        print(f"[cli] missing CLI binary: {bin_path}", file=sys.stderr)
        return 1

    try:
        repeats = int(os.environ.get("BENCH_REPEATS", "20"))
    except ValueError:
        repeats = 20

    small = FIXTURES / "uint8_128.tif"
    nac = SAMPLES / "NAC_DTM_ATLAS2.PYR.TIF"

    if not small.exists():
        print(f"[cli] missing fixture {small}", file=sys.stderr)
        return 1

    # warm-up: one throwaway invocation so dylib loads / page caches settle
    _run_once(bin_path, small)

    metrics = {
        "cli_info_uint8_128_ms": median_samples(
            repeats, lambda: _run_once(bin_path, small)
        ),
    }
    if nac.exists():
        _run_once(bin_path, nac)
        metrics["cli_info_nac_ms"] = median_samples(
            repeats, lambda: _run_once(bin_path, nac)
        )
        # Full end-to-end pixel copy of the real NAC DTM (616 tiles) through
        # the CLI read side `ptiff copy`. A throwaway destination in the OS
        # temp dir is written and removed on each sample.
        _tmp = Path(tempfile.gettempdir())
        dst = _tmp / f"ptiff_cli_copy_{os.getpid()}.tif"

        def copy_once():
            t = _run_copy_once(bin_path, nac, dst)
            try:
                dst.unlink()
            except OSError:
                pass
            return t

        copy_once()  # warm-up
        metrics["cli_copy_nac_ms"] = median_samples(repeats, copy_once)
        try:
            dst.unlink()
        except OSError:
            pass
    else:
        print("[cli] NAC sample not present; skipping cli_info_nac_ms", file=sys.stderr)

    doc = {
        "language": "cli",
        "binary": bin_path,
        "repeats": repeats,
        "metrics": metrics,
    }

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, indent=2) + "\n")
    print(json.dumps({"language": "cli", "metrics": metrics}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
