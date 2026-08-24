#!/usr/bin/env bash
# Downloads a TIFF/BigTIFF file (e.g. a real planetary imagery sample) into a local,
# gitignored cache directory, then -- if the Rust `ptiff` CLI is built -- runs it
# against the downloaded file to report whether TiffBackend can read it.
#
# Usage:
#   scripts/fetch_sample_tiff.sh <url> [output-filename]
#
# Example:
#   scripts/fetch_sample_tiff.sh https://example.org/some-lroc-tile.tif
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SAMPLES_DIR="${SCRIPT_DIR}/samples"

if [[ $# -lt 1 ]]; then
    echo "usage: $(basename "$0") <url> [output-filename]" >&2
    exit 1
fi

URL="$1"
OUTPUT_NAME="${2:-$(basename "${URL%%\?*}")}"

if [[ -z "${OUTPUT_NAME}" || "${OUTPUT_NAME}" == "/" ]]; then
    echo "error: could not derive a filename from the URL; pass one explicitly as the second argument" >&2
    exit 1
fi

mkdir -p "${SAMPLES_DIR}"
OUTPUT_PATH="${SAMPLES_DIR}/${OUTPUT_NAME}"

echo "Downloading:"
echo "  from: ${URL}"
echo "  to:   ${OUTPUT_PATH}"
curl --fail --location --progress-bar --output "${OUTPUT_PATH}" "${URL}"

SIZE_BYTES=$(wc -c < "${OUTPUT_PATH}" | tr -d ' ')
echo "Downloaded ${SIZE_BYTES} bytes."

if command -v cargo >/dev/null 2>&1 && cargo build -q -p ptiff-cli "${CARGO_FLAGS:-}"; then
    echo
    echo "Running the Rust 'ptiff' CLI against the downloaded file..."
    cargo run -q -p ptiff-cli "${CARGO_FLAGS:-}" -- info "${OUTPUT_PATH}" || true
else
    echo
    echo "Note: could not build the 'ptiff' CLI -- build it first to test-read this file:"
    echo "  cargo build -p ptiff-cli"
    echo "  target/debug/ptiff info ${OUTPUT_PATH}"
fi
