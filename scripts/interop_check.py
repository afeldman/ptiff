#!/usr/bin/env python3
"""Reproducible interop + overhead check for the PTIFF private tags (65001-65005).

Verifies the paper's central backward-compatibility claim empirically:
standard TIFF readers open the PRIMARY IMAGE of a PTIFF file unmodified,
whether or not the five private tags are present, and measures the
size/read-time overhead the tags actually add.

Prerequisite: build the sample-writing example once --
    cmake -B build/Debug -S . -DPTIFF_BUILD_EXAMPLES=ON
    cmake --build build/Debug --target ptiff_example_write_sample
This script then calls that binary to produce two otherwise-identical 64x64
tiled TIFFs -- one with the five ptiff.* domain fields set, one without --
and runs six readers against both: tiffinfo (libtiff), gdalinfo (GDAL),
identify (ImageMagick), sips (macOS), Pillow, OpenCV.

Run (from the libptiff project root, e.g. src/ptiff/):
    uv run --with pillow --with opencv-python-headless python scripts/interop_check.py
"""

from __future__ import annotations
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXAMPLE_BIN = (
    ROOT / "build" / "Debug" / "libptiff" / "examples" / "ptiff_example_write_sample"
)
OUT_DIR = ROOT / "scripts" / "results"
N_REPEATS = 20


def _build_samples() -> tuple[Path, Path]:
    if not EXAMPLE_BIN.exists():
        sys.exit(
            f"missing {EXAMPLE_BIN} -- build it first:\n"
            "  cmake -B build/Debug -S . -DPTIFF_BUILD_EXAMPLES=ON\n"
            "  cmake --build build/Debug --target ptiff_example_write_sample"
        )
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with_tags = OUT_DIR / "sample_with_tags.tif"
    plain = OUT_DIR / "sample_plain.tif"
    subprocess.run(
        [str(EXAMPLE_BIN), str(with_tags), "with-tags"], check=True, capture_output=True
    )
    subprocess.run(
        [str(EXAMPLE_BIN), str(plain), "plain"], check=True, capture_output=True
    )
    return with_tags, plain


def _time_it(fn, repeats: int = N_REPEATS) -> float:
    """Median wall-clock time in milliseconds over `repeats` calls."""
    times = []
    for _ in range(repeats):
        t0 = time.perf_counter()
        fn()
        times.append((time.perf_counter() - t0) * 1000.0)
    times.sort()
    return times[len(times) // 2]


def _check_subprocess(cmd: list[str]) -> tuple[bool, str]:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        return proc.returncode == 0, proc.stdout + proc.stderr
    except FileNotFoundError:
        return False, "NOT INSTALLED"
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT"


def check_tiffinfo(path: Path) -> dict:
    ok, out = _check_subprocess(["tiffinfo", str(path)])
    dims_ok = "Image Width: 64 Image Length: 64" in out
    tags_seen = all(f"Tag {t}:" in out for t in range(65001, 65006))
    time_ms = _time_it(
        lambda: subprocess.run(["tiffinfo", str(path)], capture_output=True)
    )
    return {
        "opens": ok,
        "dims_ok": dims_ok,
        "private_tags_listed": tags_seen,
        "time_ms": time_ms,
    }


def check_gdalinfo(path: Path) -> dict:
    if shutil.which("gdalinfo") is None:
        return {
            "opens": False,
            "dims_ok": False,
            "time_ms": None,
            "note": "gdalinfo not installed",
        }
    ok, out = _check_subprocess(["gdalinfo", str(path)])
    dims_ok = "Size is 64, 64" in out
    time_ms = _time_it(
        lambda: subprocess.run(["gdalinfo", str(path)], capture_output=True)
    )
    return {"opens": ok, "dims_ok": dims_ok, "time_ms": time_ms}


def check_identify(path: Path) -> dict:
    ok, out = _check_subprocess(["identify", str(path)])
    dims_ok = "64x64" in out
    time_ms = _time_it(
        lambda: subprocess.run(["identify", str(path)], capture_output=True)
    )
    return {"opens": ok, "dims_ok": dims_ok, "time_ms": time_ms}


def check_sips(path: Path) -> dict:
    ok, out = _check_subprocess(
        ["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(path)]
    )
    dims_ok = "pixelWidth: 64" in out and "pixelHeight: 64" in out
    time_ms = _time_it(
        lambda: subprocess.run(
            ["sips", "-g", "pixelWidth", str(path)], capture_output=True
        )
    )
    return {"opens": ok, "dims_ok": dims_ok, "time_ms": time_ms}


def check_pillow(path: Path) -> dict:
    try:
        from PIL import Image
    except ImportError:
        return {
            "opens": False,
            "dims_ok": False,
            "time_ms": None,
            "note": "Pillow not installed",
        }

    def _open():
        with Image.open(path) as im:
            im.load()

    try:
        with Image.open(path) as im:
            dims_ok = im.size == (64, 64)
        ok = True
    except Exception:
        ok, dims_ok = False, False
    time_ms = _time_it(_open) if ok else None
    return {"opens": ok, "dims_ok": dims_ok, "time_ms": time_ms}


def check_opencv(path: Path) -> dict:
    try:
        import cv2
    except ImportError:
        return {
            "opens": False,
            "dims_ok": False,
            "time_ms": None,
            "note": "OpenCV not installed",
        }

    img = cv2.imread(str(path), cv2.IMREAD_UNCHANGED)
    ok = img is not None
    dims_ok = ok and img.shape[:2] == (64, 64)
    time_ms = (
        _time_it(lambda: cv2.imread(str(path), cv2.IMREAD_UNCHANGED)) if ok else None
    )
    return {"opens": ok, "dims_ok": dims_ok, "time_ms": time_ms}


CHECKS = {
    "tiffinfo": check_tiffinfo,
    "gdalinfo": check_gdalinfo,
    "identify (ImageMagick)": check_identify,
    "sips": check_sips,
    "Pillow": check_pillow,
    "OpenCV": check_opencv,
}


def main() -> None:
    with_tags, plain = _build_samples()
    size_with_tags = with_tags.stat().st_size
    size_plain = plain.stat().st_size
    size_overhead = size_with_tags - size_plain

    results: dict = {
        "size_bytes": {
            "plain": size_plain,
            "with_tags": size_with_tags,
            "overhead": size_overhead,
        },
        "readers": {},
    }
    print(
        f"Size overhead of 5 ptiff.* tags: {size_overhead} bytes "
        f"({size_plain} -> {size_with_tags})\n"
    )
    print(
        f"{'Reader':<24}{'opens (plain)':<15}{'opens (+tags)':<15}"
        f"{'dims ok':<10}{'time plain(ms)':<16}{'time +tags(ms)':<16}"
    )
    for name, fn in CHECKS.items():
        r_plain = fn(plain)
        r_tags = fn(with_tags)
        results["readers"][name] = {"plain": r_plain, "with_tags": r_tags}
        tp = (
            f"{r_plain['time_ms']:.3f}" if r_plain.get("time_ms") is not None else "n/a"
        )
        tt = f"{r_tags['time_ms']:.3f}" if r_tags.get("time_ms") is not None else "n/a"
        print(
            f"{name:<24}{str(r_plain['opens']):<15}{str(r_tags['opens']):<15}"
            f"{str(r_tags['dims_ok']):<10}{tp:<16}{tt:<16}"
        )

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out_path = OUT_DIR / "interop_check.json"
    out_path.write_text(json.dumps(results, indent=2))
    print(f"\nSaved: {out_path}")


if __name__ == "__main__":
    main()
