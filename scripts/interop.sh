#!/usr/bin/env bash
# P-1 (documents/paper/ACTION_PLAN.md): interoperability + overhead experiment.
#
# 1. Generates fixtures via the Rust example `write_ptiff_fixture`
#    (PTIFF TIFF with/without the 5 PTIFF private tags) plus GeoTIFF
#    (+GDAL_METADATA) and TIFF+sidecar variants for a size/overhead
#    comparison.
# 2. Reads the PTIFF fixture with tiffinfo, gdalinfo, ImageMagick identify,
#    macOS sips, Python Pillow and OpenCV -- logs expected vs. actual.
# 3. Prints a size/overhead table and a decode-time sample (gdalinfo, median
#    of N runs).
#
# All findings are reported as measured, including negative ones (e.g. ISIS3
# silently dropping unknown metadata, GDAL refusing our minimal PDS4/ISIS
# labels) -- see documents/paper/ptiff/ptiff.tex.
#
# Usage: scripts/interop.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SAMPLES=scripts/samples
RESULTS=scripts/results
mkdir -p "$SAMPLES" "$RESULTS"
LOG="$RESULTS/interop.log"
: > "$LOG"

log() { echo "$@" | tee -a "$LOG"; }

log "== Generate PTIFF fixtures via the idiomatic Rust example =="
# The C++ gen_interop_fixture was removed with libptiff (2026-08-24); the PTIFF
# fixtures are now written by the Rust example `write_ptiff_fixture` (a 64x64
# tiled TIFF with/without the camera/CRS private tags).
RUST_EX="cargo run -q -p ptiff --example write_ptiff_fixture --"
$RUST_EX "$SAMPLES/fixture_tiff_meta.bin" with-tags | tee -a "$LOG"
$RUST_EX "$SAMPLES/fixture_tiff_nometa.bin" plain | tee -a "$LOG"
# PDS4/ISIS3/GeoTIFF fixture generation (backends not exposed by the idiomatic
# crate) is a deferred paper follow-up; the GDAL cross-check below is skipped.

log ""
log "== GeoTIFF (+GDAL_METADATA) and TIFF+sidecar variants =="
gdal_translate "$SAMPLES/fixture_tiff_nometa.bin" "$SAMPLES/fixture_geotiff_nometa.tif" >>"$LOG" 2>&1
gdal_translate "$SAMPLES/fixture_tiff_nometa.bin" "$SAMPLES/fixture_geotiff_meta.tif" \
    -mo "ptiff.spice.frame=IAU_MOON" \
    -mo "ptiff.spice.instrument=LROC_NAC" \
    -mo "ptiff.spice.time_system=TDB" \
    -mo "ptiff.spice.observation_time=2026-08-20T00:00:00.000" \
    -mo "ptiff.camera.model=pinhole" \
    -mo "ptiff.camera.focal_length_x=700.0" \
    -mo "ptiff.camera.focal_length_y=700.0" \
    -mo "ptiff.camera.principal_x=64.000000" \
    -mo "ptiff.camera.principal_y=64.000000" \
    -mo "ptiff.crs.body=301" \
    -mo "ptiff.crs.projection=equirectangular" \
    -mo "ptiff.crs.reference_frame=IAU_MOON_2000" \
    -mo "ptiff.layers.dem=dem_128x128" \
    -mo "ptiff.layers.confidence=confidence_128x128" \
    -mo "ptiff.provenance.software=libptiff-0.3.0" \
    -mo "ptiff.provenance.operator=interop-fixture-generator" \
    -mo "ptiff.provenance.commit=f73dec27469f878a8de9d209e0c81bf33e8b8d0a" \
    >>"$LOG" 2>&1
cat > "$SAMPLES/fixture_sidecar_meta.json" << 'JSON'
{
  "ptiff.spice.frame": "IAU_MOON",
  "ptiff.spice.instrument": "LROC_NAC",
  "ptiff.spice.time_system": "TDB",
  "ptiff.spice.observation_time": "2026-08-20T00:00:00.000",
  "ptiff.camera.model": "pinhole",
  "ptiff.camera.focal_length_x": "700.0",
  "ptiff.camera.focal_length_y": "700.0",
  "ptiff.camera.principal_x": "64.000000",
  "ptiff.camera.principal_y": "64.000000",
  "ptiff.crs.body": "301",
  "ptiff.crs.projection": "equirectangular",
  "ptiff.crs.reference_frame": "IAU_MOON_2000",
  "ptiff.layers.dem": "dem_128x128",
  "ptiff.layers.confidence": "confidence_128x128",
  "ptiff.provenance.software": "libptiff-0.3.0",
  "ptiff.provenance.operator": "interop-fixture-generator",
  "ptiff.provenance.commit": "f73dec27469f878a8de9d209e0c81bf33e8b8d0a"
}
JSON

log ""
log "== Multi-tool read of the PTIFF fixture (fixture_tiff_meta.bin) =="
F="$SAMPLES/fixture_tiff_meta.bin"
log "-- tiffinfo --"; tiffinfo "$F" >>"$LOG" 2>&1 || log "  FAILED"
log "-- gdalinfo --"; gdalinfo "$F" >>"$LOG" 2>&1 || log "  FAILED"
log "-- ImageMagick identify --"; identify "$F" >>"$LOG" 2>&1 || log "  FAILED"
log "-- sips --"; sips -g all "$F" >>"$LOG" 2>&1 || log "  FAILED"
log "-- Pillow --"
python3 -c "
from PIL import Image
im = Image.open('$F'); im.load()
print('PIL OK', im.size, im.mode)
" >>"$LOG" 2>&1 || log "  FAILED"
log "-- OpenCV (ephemeral uv env) --"
uv run --with opencv-python-headless --with numpy python3 -c "
import cv2
img = cv2.imread('$F', cv2.IMREAD_UNCHANGED)
print('cv2 OK' if img is not None else 'cv2 FAILED', None if img is None else img.shape)
" >>"$LOG" 2>&1 || log "  FAILED"

log ""
log "== Cross-check: can GDAL open the (deferred) PDS4/ISIS output? =="
# The PDS4/ISIS fixture writers (backend-specific, C++) were removed with
# libptiff; regenerating them from Rust is a deferred paper follow-up. The
# GDAL cross-check below therefore runs only if a fixture already exists.
if [ -f "$SAMPLES/fixture_pds4_meta.bin" ]; then
  if gdalinfo "$SAMPLES/fixture_pds4_meta.bin" >>"$LOG" 2>&1; then
    log "pds4: opened"
  else
    log "pds4: GDAL refused (expected -- minimal, non-standard label schema, see paper)"
  fi
else
  log "pds4: skipped (fixture writer deferred to Rust)"
fi
if [ -f "$SAMPLES/fixture_isis_meta.bin" ]; then
  if gdalinfo "$SAMPLES/fixture_isis_meta.bin" >>"$LOG" 2>&1; then
    log "isis: opened"
  else
    log "isis: GDAL refused (expected -- minimal, non-standard label schema, see paper)"
  fi
else
  log "isis: skipped (fixture writer deferred to Rust)"
fi

log ""
log "== Size/overhead table (64x64 UInt8, identical logical metadata) =="
printf "%-30s %10s %10s %10s\n" "Variant" "no-meta(B)" "meta(B)" "overhead(B)" | tee -a "$LOG"
size() { stat -f%z "$1" 2>/dev/null || stat -c%s "$1"; }
pair="PTIFF (tiff):fixture_tiff"
label="${pair%%:*}"; base="${pair##*:}"
n=$(size "$SAMPLES/${base}_nometa.bin"); m=$(size "$SAMPLES/${base}_meta.bin")
printf "%-30s %10s %10s %10s\n" "$label" "$n" "$m" "$((m - n))" | tee -a "$LOG"
n=$(size "$SAMPLES/fixture_geotiff_nometa.tif"); m=$(size "$SAMPLES/fixture_geotiff_meta.tif")
printf "%-30s %10s %10s %10s\n" "GeoTIFF (GDAL_METADATA)" "$n" "$m" "$((m - n))" | tee -a "$LOG"
sc=$(size "$SAMPLES/fixture_sidecar_meta.json")
printf "%-30s %10s %10s %10s (sidecar file, not embedded)\n" "TIFF + JSON sidecar" "$n" "$((n + sc))" "$sc" | tee -a "$LOG"

log ""
log "== Decode/open time (gdalinfo, median of 30 runs) =="
python3 -c "
import subprocess, time
files = {
    'PTIFF (tiff+5 tags)': '$SAMPLES/fixture_tiff_meta.bin',
    'TIFF (no meta)': '$SAMPLES/fixture_tiff_nometa.bin',
    'GeoTIFF (+GDAL_METADATA)': '$SAMPLES/fixture_geotiff_meta.tif',
    'GeoTIFF (no meta)': '$SAMPLES/fixture_geotiff_nometa.tif',
}
N = 30
for label, path in files.items():
    times = []
    for _ in range(N):
        t0 = time.perf_counter()
        subprocess.run(['gdalinfo', path], capture_output=True, check=True)
        times.append(time.perf_counter() - t0)
    times.sort()
    print(f'{label:30s} median={times[N//2]*1000:.2f} ms  min={min(times)*1000:.2f} ms')
" | tee -a "$LOG"

log ""
log "Full log: $LOG"
