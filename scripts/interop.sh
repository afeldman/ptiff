#!/usr/bin/env bash
# P-1 (documents/paper/ACTION_PLAN.md): interoperability + overhead experiment.
#
# 1. Builds gen_interop_fixture (PTIFF_BUILD_INTEROP_SCRIPTS) and generates real,
#    complete fixtures (TIFF/PDS4/ISIS3, with and without the 5 PTIFF private
#    tags / equivalent metadata) plus GeoTIFF(+GDAL_METADATA) and TIFF+sidecar
#    variants for a size/overhead comparison.
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

BUILD_DIR=build/Debug
SAMPLES=scripts/samples
RESULTS=scripts/results
mkdir -p "$SAMPLES" "$RESULTS"
LOG="$RESULTS/interop.log"
: > "$LOG"

log() { echo "$@" | tee -a "$LOG"; }

log "== Build gen_interop_fixture =="
cmake --preset conan-debug -DPTIFF_BUILD_INTEROP_SCRIPTS=ON >>"$LOG" 2>&1
cmake --build --preset conan-debug --target gen_interop_fixture >>"$LOG" 2>&1
GEN="$BUILD_DIR/scripts/gen_interop_fixture"

log ""
log "== Generate fixtures (128x128, tiled single-tile, gradient pixels) =="
for b in tiff pds4 isis; do
    "$GEN" "$b" "$SAMPLES/fixture_${b}_meta.bin" 128 128 | tee -a "$LOG"
done
for b in tiff pds4 isis; do
    "$GEN" "$b" "$SAMPLES/fixture_${b}_nometa.bin" 128 128 --no-ptiff-fields | tee -a "$LOG"
done

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
log "== Cross-check: can GDAL open our PDS4/ISIS backend output at all? =="
if gdalinfo "$SAMPLES/fixture_pds4_meta.bin" >>"$LOG" 2>&1; then
  log "pds4: opened"
else
  log "pds4: GDAL refused (expected -- minimal, non-standard label schema, see paper)"
fi
if gdalinfo "$SAMPLES/fixture_isis_meta.bin" >>"$LOG" 2>&1; then
  log "isis: opened"
else
  log "isis: GDAL refused (expected -- minimal, non-standard label schema, see paper)"
fi

log ""
log "== Size/overhead table (128x128 UInt8, identical logical metadata) =="
printf "%-30s %10s %10s %10s\n" "Variant" "no-meta(B)" "meta(B)" "overhead(B)" | tee -a "$LOG"
size() { stat -f%z "$1" 2>/dev/null || stat -c%s "$1"; }
for pair in "PTIFF (tiff):fixture_tiff" "PDS4+RAW:fixture_pds4" "ISIS3-CUB:fixture_isis"; do
    label="${pair%%:*}"; base="${pair##*:}"
    n=$(size "$SAMPLES/${base}_nometa.bin"); m=$(size "$SAMPLES/${base}_meta.bin")
    printf "%-30s %10s %10s %10s\n" "$label" "$n" "$m" "$((m - n))" | tee -a "$LOG"
done
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
