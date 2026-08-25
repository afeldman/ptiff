#!/usr/bin/env python3
"""Summarize per-language benchmark JSONs into a comparison table.

Aggregates every `*.json` in a results directory (default: benchmark-results/)
produced by benchmarks/src/bench_{python,ruby,go,octave,rust}.* and prints a
Markdown table of median times (ms) -- one row per metric, one column per
language -- plus a CSV copy, writing both next to the JSONs.

Metric names differ slightly between the SWIG scripts and the Rust benchmark
(Rust has no per-tile read; it reports read_metadata_* instead). Rows below the
"---" line are therefore flagged as not pixel-read for Rust.

Usage:
    python3 src/summary.py [results-dir]     # default benchmark-results/
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

DEFAULT_DIR = Path(__file__).resolve().parent.parent / "benchmark-results"

# Canonical metrics to show (order matters). Keys map a readable label to the
# underlying per-language metric key.
METRICS = [
    ("write_all_tiles", "write_all_tiles_ms"),
    ("read_uint8_128", "read_uint8_128_ms"),
    ("read_uint8_512", "read_uint8_512_ms"),
    ("read_f32_512", "read_f32_512_ms"),
    # --real mode (genuine NASA LOLA crop; only present when run with --real):
    ("read_real_lola_512", "read_real_lola_512_ms"),
    # --nac mode (full real NASA LRO-NAC DTM, 616 tiles; only with --nac):
    ("read_nac_616", "read_nac_ms"),
    # Rust-only (metadata open, no pixel decode):
    ("read_metadata_uint8_128", "read_metadata_uint8_128_ms"),
    ("read_metadata_uint8_512", "read_metadata_uint8_512_ms"),
    ("read_metadata_f32_512", "read_metadata_f32_512_ms"),
    ("read_metadata_real_lola_512", "read_metadata_real_lola_512_ms"),
    ("read_metadata_nac", "read_metadata_nac_ms"),
    # CLI (fresh-subprocess `ptiff info <file>` end-to-end incl. library load):
    ("cli_info_uint8_128", "cli_info_uint8_128_ms"),
    ("cli_info_nac", "cli_info_nac_ms"),
    # CLI full end-to-end pixel copy of the real NAC DTM (fresh subprocess
    # `ptiff copy`, read+write all 616 tiles; only when the NAC sample exists):
    ("cli_copy_nac", "cli_copy_nac_ms"),
]

ORDER = ["python", "ruby", "go", "octave", "julia", "rust", "cli"]

# Exact pixel payload (bytes of decoded tile data) read by each *pixel-read*
# metric with --nac / --real / default fixtures. Used to compute a MB/s
# throughput companion row. Derived from the deterministic fixture geometry:
#   uint8_128     = 128*128*1
#   uint8_512     = 512*512*1
#   f32_512/real  = 512*512*4
#   nac           = 2693*14236 (UInt8, 616 tiles) == ceil(2693/256)*ceil(14236/256)
PAYLOAD_BYTES = {
    "read_uint8_128_ms": 128 * 128 * 1,
    "read_uint8_512_ms": 512 * 512 * 1,
    "read_f32_512_ms": 512 * 512 * 4,
    "read_real_lola_512_ms": 512 * 512 * 4,
    "read_nac_ms": 2693 * 14236,
}


def load(results_dir: Path) -> dict[str, dict]:
    docs: dict[str, dict] = {}
    for p in sorted(results_dir.glob("*.json")):
        if p.name.startswith("summary"):
            continue
        try:
            doc = json.loads(p.read_text())
        except (json.JSONDecodeError, OSError) as e:
            print(f"[summary] skip {p.name}: {e}", file=sys.stderr)
            continue
        lang = doc.get("language")
        if lang:
            docs[lang] = doc
    return docs


def cell(doc: dict | None, metric_key: str) -> str:
    if doc is None:
        return "n/a"
    m = doc.get("metrics", {}).get(metric_key)
    if not m:
        return "n/a"
    med = m.get("median_ms")
    return f"{med:.3f}" if med is not None else "n/a"


def throughput_cell(doc: dict | None, metric_key: str, payload_bytes: int) -> str:
    """MB/s of decoded pixel data for a pixel-read metric (payload_bytes /
    median_ms). Returns 'n/a' when the metric is absent (e.g. non-read/CLI/rust
    metadata-only metrics), so a companion row stays aligned column-wise."""
    if doc is None:
        return "n/a"
    m = doc.get("metrics", {}).get(metric_key)
    if not m:
        return "n/a"
    med = m.get("median_ms")
    if med is None or med <= 0:
        return "n/a"
    return f"{payload_bytes / (med / 1000.0) / 1e6:.1f}"


def main() -> int:
    results_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_DIR
    docs = load(results_dir)
    if not docs:
        print(f"[summary] no benchmark JSONs found in {results_dir}", file=sys.stderr)
        return 1

    langs = [lang for lang in ORDER if lang in docs]
    langs += [
        lang for lang in docs if lang not in ORDER
    ]  # unexpected extras, keep anyway

    # Markdown
    lines = [
        "# ptiff binding benchmarks — median times (ms)",
        "",
        f"Updated {__import__('datetime').datetime.now().isoformat(timespec='minutes')}",
        "",
        f"{REPEATS_LABEL(docs)}",
        "",
        "| metric | " + " | ".join(f"**{lang}**" for lang in langs) + " |",
        "|" + "---|" * (1 + len(langs)),
    ]
    note_rust = False
    note_cli = False
    for label, key in METRICS:
        row = [label] + [cell(docs.get(lang), key) for lang in langs]
        lines.append("| " + " | ".join(row) + " |")
        if key.startswith("read_metadata_"):
            note_rust = True
        if key.startswith("cli"):
            note_cli = True
        # Pixel-read metrics get a companion MB/s throughput row.
        payload = PAYLOAD_BYTES.get(key)
        if payload:
            trow = [f"{label} MB/s"] + [
                throughput_cell(docs.get(lang), key, payload) for lang in langs
            ]
            lines.append("| " + " | ".join(trow) + " |")
    if note_rust:
        lines.append("")
        lines.append(
            "_read_metadata_*: Rust metadata-open only (no per-tile pixel read in the binding)._"
        )
    if note_cli:
        lines.append("")
        lines.append(
            "_cli_info_*: fresh-subprocess `ptiff info <file>` (end-to-end, incl. process startup + library load)._"
        )
    md = "\n".join(lines)

    # CSV
    csv_rows: list[list[str]] = [["metric"] + langs]
    for label, key in METRICS:
        csv_rows.append([label] + [cell(docs.get(lang), key) for lang in langs])
        payload = PAYLOAD_BYTES.get(key)
        if payload:
            csv_rows.append(
                [f"{label} MB/s"]
                + [throughput_cell(docs.get(lang), key, payload) for lang in langs]
            )

    md_path = results_dir / "summary.md"
    csv_path = results_dir / "summary.csv"
    md_path.write_text(md + "\n")
    with csv_path.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerows(csv_rows)

    print(md)
    print(f"\n[summary] wrote {md_path} and {csv_path}")
    return 0


def REPEATS_LABEL(docs: dict) -> str:
    # The CLI doc has no iterations_per_sample (subprocess lifetime is the
    # unit, no inner loop); prefer a language doc that does.
    sample_doc = next(
        (d for d in docs.values() if "iterations_per_sample" in d),
        next(iter(docs.values()), {}),
    )
    rep = sample_doc.get("repeats", "?")
    it = sample_doc.get("iterations_per_sample", "?")
    return f"_repeats={rep}, iterations-per-sample={it} (see each JSON for the exact per-run sample)_"


if __name__ == "__main__":
    raise SystemExit(main())
