#!/usr/bin/env python3
# examples/fetch_esa_nasa_data.py
#
# Downloads real ESA and NASA raster datasets (as Cloud-Optimized GeoTIFF /
# BigTIFF) into examples/data/ so the Rust example can read them with `ptiff`.
#
# Sources (Microsoft Planetary Computer; all public, licensed for reuse):
#
#   ESA   : Sentinel-2 L2A "visual"          (the iconic Copernicus programme
#           product; 10 m true-colour RGB, produced under ESA/EU ).
#           Collection: sentinel-2-l2a
#   NASA  : ASTER L1T VNIR                   (ASTER instrument on NASA's Terra
#           satellite; orthorectified VNIR reflectance GeoTIFF).
#           Collection: aster-l1t
#
# The Planetary Computer requires its asset URLs to be *signed* with a short-lived
# SAS token before they can be read. This script uses the `planetary-computer` SDK
# to discover and sign the real asset URL, then streams the bytes with urllib.
#
# Usage:
#   python3 fetch_esa_nasa_data.py            # download both ESA + NASA samples
#   python3 fetch_esa_nasa_data.py --collection sentinel-2
#   python3 fetch_esa_nasa_data.py --data-dir /some/where
#
# Requires:   python3, planetary-computer, pystac-client  (pip install ...)

from __future__ import annotations

import argparse
import hashlib
import sys
import urllib.error
import urllib.request
from pathlib import Path

try:
    import planetary_computer as pc
    import pystac_client
except ImportError as exc:  # pragma: no cover
    sys.exit(
        f"missing dependency: {exc}. Install it with:\n"
        "  python3 -m pip install planetary-computer pystac-client\n"
        "or run inside the bundled environment: python3 fetch_esa_nasa_data.py"
    )

STAC_API = "https://planetarycomputer.microsoft.com/api/stac/v1"

# ---------------------------------------------------------------------------
# Sample definitions: one (collection, preferred asset) per real dataset.
# The item is looked up dynamically so we always point at the current granule.
# ---------------------------------------------------------------------------
SAMPLES = {
    # ESA - Sentinel-2 L2A "visual" = true-colour RGB (10 m) COG GeoTIFF.
    # This is the iconic Copernicus/ESA product, and unlike the float32 DEM
    # tiles (Predictor 3) it reads cleanly with `ptiff`.
    "sentinel-2": {
        "collection": "sentinel-2-l2a",
        "asset": "visual",
        "filename": "sentinel2_l2a_visual_rgb.tif",
        "label": "ESA  Sentinel-2 L2A visual (Copernicus, true-colour RGB GeoTIFF)",
    },
    # NASA - ASTER L1T (orthorectified radiance, Terra/ASTER; COG GeoTIFF).
    "aster": {
        "collection": "aster-l1t",
        "asset": "VNIR",
        "filename": "aster_l1t.tif",
        "label": "NASA ASTER L1T VNIR (Terra, orthorectified GeoTIFF)",
    },
}


def _pick_item(collection: str):
    """Return the first available STAC item for a Planetary Computer collection."""
    catalog = pystac_client.Client.open(STAC_API)
    search = catalog.search(collections=[collection], max_items=1)
    try:
        return next(iter(search.items()))
    except StopIteration:
        sys.exit(f"error: no items found for collection '{collection}'")
    finally:
        catalog = None


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(sample: dict, data_dir: Path, *, force: bool = False) -> Path:
    label = sample["label"]
    coll = sample["collection"]
    dest = data_dir / sample["filename"]

    if dest.exists() and not force:
        print(f"  [cached] {label}")
        print(f"           {dest.name} ({dest.stat().st_size} bytes)")
        return dest

    print(f"  [fetch ] {label}")
    item = _pick_item(coll)
    signed = pc.sign(item)
    assets = {k: v.href for k, v in signed.assets.items()}
    key = sample["asset"]
    if key not in assets:
        # Fall back to the first raster-ish asset if the preferred one is absent.
        key = next(iter(assets))
        print(f"           (asset '{sample['asset']}' missing; using '{key}')")
    href = assets[key]

    print(f"           collection: {coll}")
    print(f"           item:       {item.id}")
    print(f"           asset:      {key}")
    print(f"           url:        {href[:110]}...")

    dest.parent.mkdir(parents=True, exist_ok=True)
    try:
        with (
            urllib.request.urlopen(urllib.request.Request(href), timeout=300) as resp,
            open(dest, "wb") as out,
        ):
            total = 0
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                out.write(chunk)
                total += len(chunk)
    except (urllib.error.URLError, OSError, ValueError) as exc:
        dest.unlink(missing_ok=True)
        sys.exit(f"error: download failed for '{dest.name}': {exc}")

    print(f"           saved: {dest} ({dest.stat().st_size} bytes)")
    return dest


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Download real ESA + NASA GeoTIFF samples for the examples/ directory."
    )
    ap.add_argument(
        "--collection",
        "-c",
        default=None,
        choices=list(SAMPLES.keys()),
        help="Download only one sample (default: all).",
    )
    ap.add_argument(
        "--data-dir",
        default=str(Path(__file__).resolve().parent / "data"),
        help="Directory to store downloaded files (default: examples/data).",
    )
    ap.add_argument(
        "--force", "-f", action="store_true", help="Re-download even if cached."
    )
    args = ap.parse_args()

    data_dir = Path(args.data_dir).resolve()
    data_dir.mkdir(parents=True, exist_ok=True)

    keys = [args.collection] if args.collection else list(SAMPLES.keys())
    print(f"Fetching {len(keys)} sample(s) into {data_dir}\n")
    results = {}
    for key in keys:
        path = download(SAMPLES[key], data_dir, force=args.force)
        results[key] = path

    print("\nDone. Downloaded files:")
    for key, path in results.items():
        print(
            f"  {key:20s} -> {path.name}  ({path.stat().st_size} bytes, "
            f"sha256 {_sha256(path)[:12]}…)"
        )

    print(
        "\nNext step — build & run the Rust reader example:\n"
        "  cargo run -p ptiff --example read_esa_nasa_tiff -- <path-to-sample>.tif"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
