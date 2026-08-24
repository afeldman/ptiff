#!/usr/bin/env bash
# run_benchmarks.sh -- run the ptiff cross-language benchmark suite.
#
# Generates the shared fixtures (benchmarks/fixtures), then runs one benchmark
# script per language binding (Python, Ruby, Go, Octave, Rust) over the same
# read/write metric set with median-of-repeats timing. Each script writes its
# per-language JSON into benchmark-results/<lang>.json; summary.py then folds
# them into the Markdown table (+ CSV) you use for the "Auswertung".
#
# Requirements (all four are built by the binding CI / README flows):
#   - PYTHONPATH binding: bindings/python/src  (ptiff + _ptiff.so)
#   - Ruby binding:       bindings/ruby/lib    (ptiff.bundle)
#   - Octave binding:     bindings/octave/lib  (ptiff.oct)
#   - Go binding:         bindings/go  (cgo; needs libptiff_c on pkg-config)
#   - Rust binding:       bindings/rust + this crate (rust_bench/)
#
# The C ABI libptiff_c is resolved per language: Python/Ruby/Octave dylibs ship
# an rpath so they work out-of-the-box; Go and Rust need a pkg-config prefix
# (default: $REPO/install-shared). Override with PTIFF_PREFIX (an install
# directory containing lib/pkgconfig) or the usual per-binding env vars.
#
# Usage:
#   benchmarks/run_benchmarks.sh              # full suite (all languages)
#   benchmarks/run_benchmarks.sh python ruby  # subset
#   benchmarks/run_benchmarks.sh --real       # add real NASA data (LOLA crop) reads
#   benchmarks/run_benchmarks.sh --nac        # add a real NASA NAC-DTM (55 MB) read
#   BENCH_REPEATS=10 BENCH_ITERS=25 benchmarks/run_benchmarks.sh
set -euo pipefail

cd "$(dirname "$0")"

REPO="$(cd .. && pwd)"
OUT="benchmark-results"
mkdir -p "$OUT"

# `--real` adds a read of a genuine NASA LOLA elevation crop; `--nac` adds a
# read of the full ~55 MB LRO-NAC DTM pyramid (scripts/samples/). Both are off
# by default because they need the gitignored scripts/samples/ present (fetch
# via scripts/fetch_sample_tiff.sh). `--nac` also feeds the CLI's large-file
# metadata-open (cli_info_nac_ms).
REAL=0
NAC=0
LANG_ARGS=()
for a in "$@"; do
  case "$a" in
    --real) REAL=1 ;;
    --nac)  NAC=1 ;;
    *) LANG_ARGS+=("$a") ;;
  esac
done

# Tuning knobs passed through to every script.
export BENCH_REPEATS="${BENCH_REPEATS:-20}"
export BENCH_ITERS="${BENCH_ITERS:-50}"
# Inner iterations for the heavy NAC metric (616 tiles/pass); a small default
# keeps the big-file read cheap even at the default BENCH_REPEATS.
export BENCH_NAC_ITERS="${BENCH_NAC_ITERS:-2}"

# pkg-config prefix for the Go/Rust bindings (install dir with lib/pkgconfig).
PTIFF_PREFIX="${PTIFF_PREFIX:-$REPO/install-shared}"
PKGCONF="$PTIFF_PREFIX/lib/pkgconfig"
LIBDIR="$PTIFF_PREFIX/lib"
# Flags passed through to every benchmark: `--real` / `--nac` appear only if
# the corresponding option was requested (the expansion intentionally word-splits,
# so use an array rather than the unquoted $(...) && echo idiom).
EXTRA_ARGS=()
[ "$REAL" -eq 1 ] && EXTRA_ARGS+=(--real)
[ "$NAC" -eq 1 ] && EXTRA_ARGS+=(--nac)

log() { printf '[bench] %s\n' "$*"; }
fail() { printf '[bench] WARN: %s\n' "$*"; }

# ---- 0. fixtures ----
# Regenerate fixtures fresh each run so measurements are over identical files
# regardless of previous writes/size drift.
if [ ! -d "$REPO/bindings/python/src" ]; then
  fail "Python binding missing (bindings/python/src) -- run 'make -C bindings/swig python' first."
fi
log "generating fixtures"
PYTHONPATH="$REPO/bindings/python/src" python3 src/make_fixtures.py

# --real: stage a copy of the real NASA LOLA elevation crop into fixtures/ so
# every language reads the identical real-data byte stream. (Same deterministic
# name; absent = '--real' runs degrade gracefully to the synthetic suite only.)
if [ "$REAL" -eq 1 ]; then
  LOLA_SRC="$REPO/scripts/samples/lola_real_crop_512.tif"
  if [ -f "$LOLA_SRC" ]; then
    cp "$LOLA_SRC" fixtures/real_lola_512.tif
    log "staged real sample: scripts/samples/lola_real_crop_512.tif -> fixtures/real_lola_512.tif"
  else
    fail "--real requested but $LOLA_SRC missing (run scripts/fetch_sample_tiff.sh?)"
  fi
fi

# --nac: stage the full NASA LRO-NAC DTM (2693x14236 UInt8, 256x256 tiles,
# ~55 MB) into fixtures/ so every language reads the identical real-data byte
# stream (616 tiles/pass). Skipped gracefully if the sample is not cached.
if [ "$NAC" -eq 1 ]; then
  NAC_SRC="$REPO/scripts/samples/NAC_DTM_ATLAS2.PYR.TIF"
  if [ -f "$NAC_SRC" ]; then
    cp "$NAC_SRC" fixtures/nac_dtm.tif
    log "staged real NAC sample: scripts/samples/NAC_DTM_ATLAS2.PYR.TIF -> fixtures/nac_dtm.tif"
  else
    fail "--nac requested but $NAC_SRC missing (run scripts/fetch_sample_tiff.sh?)"
  fi
fi

# ---- helpers ----
run_python() {
  log "benchmarking python"
  PYTHONPATH="$REPO/bindings/python/src" \
    python3 src/bench_python.py --out "$OUT/python.json" \
      "${EXTRA_ARGS[@]}"
}

run_ruby() {
  log "benchmarking ruby"
  if [ ! -d "$REPO/bindings/ruby/lib" ]; then fail "ruby binding missing"; return 1; fi
  RUBYLIB="$REPO/bindings/ruby/lib" \
    ruby src/bench_ruby.rb --out "$OUT/ruby.json" \
      "${EXTRA_ARGS[@]}"
}

run_go() {
  log "benchmarking go"
  if ! command -v go >/dev/null; then fail "go not installed"; return 1; fi
  if [ ! -f "$PKGCONF/libptiff_c.pc" ]; then fail "no libptiff_c.pc under $PKGCONF"; return 1; fi
  PKG_CONFIG_PATH="$PKGCONF" \
    go run ./src/bench_go.go --out "$OUT/go.json" \
      "${EXTRA_ARGS[@]}"
}

run_octave() {
  log "benchmarking octave"
  if ! command -v octave >/dev/null; then fail "octave not installed"; return 1; fi
  if [ ! -d "$REPO/bindings/octave/lib" ]; then fail "octave binding missing"; return 1; fi
  octave --quiet --no-gui \
    --eval "addpath('$REPO/bindings/octave/lib','$REPO/benchmarks/src'); bench_octave('$REPO/benchmarks/$OUT/octave.json', $([ "$REAL" = 1 ] && echo true || echo false), $([ "$NAC" = 1 ] && echo true || echo false));"
}

run_rust() {
  log "benchmarking rust"
  if ! command -v cargo >/dev/null; then fail "cargo not installed"; return 1; fi
  if [ ! -f "$PKGCONF/libptiff_c.pc" ]; then fail "no libptiff_c.pc under $PKGCONF"; return 1; fi
  PKG_CONFIG_PATH="$PKGCONF" DYLD_LIBRARY_PATH="$LIBDIR" \
    cargo run --release --manifest-path rust_bench/Cargo.toml -- \
      --out "$OUT/rust.json" \
      "${EXTRA_ARGS[@]}"
}

run_cli() {
  log "benchmarking ptiff CLI (end-to-end subprocess)"
  if ! command -v cargo >/dev/null; then fail "cargo not installed (needed to build CLI)"; return 1; fi
  if [ ! -f "$PKGCONF/libptiff_c.pc" ]; then fail "no libptiff_c.pc under $PKGCONF"; return 1; fi
  local CLI_BIN
  local CLI_BIN_RELEASE="$REPO/ptiff-cli/target/release/ptiff"
  local CLI_BIN_DEBUG="$REPO/ptiff-cli/target/debug/ptiff"
  if [ -x "$CLI_BIN_RELEASE" ]; then
    CLI_BIN="$CLI_BIN_RELEASE"
  elif [ -x "$CLI_BIN_DEBUG" ]; then
    CLI_BIN="$CLI_BIN_DEBUG"
  else
    log "building ptiff CLI (ptiff-cli/)"
    (cd "$REPO/ptiff-cli" && PKG_CONFIG_PATH="$PKGCONF" cargo build --release) \
      || { fail "ptiff CLI build failed"; return 1; }
    CLI_BIN="$CLI_BIN_RELEASE"
  fi
  PYTHONPATH="$REPO/bindings/python/src" \
    DYLD_LIBRARY_PATH="$LIBDIR" \
    python3 src/bench_cli.py --bin "$CLI_BIN" --out "$OUT/cli.json"
}

# ---- dispatch ----
LANG_SELECT="${LANG_ARGS[*]:-python ruby go octave rust cli}"
status=0
for lang in $LANG_SELECT; do
  case "$lang" in
    python) run_python ;;
    ruby)   run_ruby ;;
    go)     run_go ;;
    octave) run_octave ;;
    rust)   run_rust ;;
    cli)    run_cli ;;
    *) fail "unknown target '$lang' (python|ruby|go|octave|rust|cli)"; status=1 ;;
  esac || { fail "$lang benchmark failed (continued)"; status=1; }
done

# ---- final summary ----
log "generating summary table"
if python3 src/summary.py "$OUT"; then
  log "summary written: $OUT/summary.md and $OUT/summary.csv"
else
  fail "summary step failed (raw JSON still in $OUT/)"
  status=1
fi
# Re-raise so `set -e` users see a failure if any sub-benchmark died.
[ "$status" -eq 0 ] || exit 1
