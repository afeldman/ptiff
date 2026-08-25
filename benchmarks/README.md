# ptiff — cross-language benchmark suite

Reproducible read/write-throughput benchmarks over **every language binding**
(Python, Ruby, Go, Octave, Rust), measured against the *same* C ABI
(`libptiff_c`) and the *same* on-disk fixtures, with median-of-repeats timing.
This is the "Auswertung" home: each run produces per-language JSON raw data and
a Markdown/CSV summary table you can diff across commits or across hosts.

## Layout

```
benchmarks/
├── run_benchmarks.sh          # orchestrator: fixtures -> per-language runs -> summary
├── src/
│   ├── make_fixtures.py       # writes benchmarks/fixtures/*.tif (deterministic)
│   ├── bench_python.py        # SWIG Python binding benchmark
│   ├── bench_ruby.rb          # SWIG Ruby binding benchmark
│   ├── bench_go.go            # SWIG Go binding benchmark  (+ benchmarks/go.mod)
│   ├── bench_octave.m         # Octave MEX binding benchmark
│   ├── bench_julia.jl         # Ptiff.jl (ccall) binding benchmark
│   ├── bench_cli.py           # ptiff CLI end-to-end (fresh subprocess) benchmark
│   └── summary.py             # folds per-language JSONs into summary.md/.csv
├── rust_bench/                # Rust binding benchmark crate (Cargo project)
├── parallel_bench/            # Phase-5 Rayon scaling benchmark (ptiff-core, 32768 tiles)
├── fixtures/                  # GENERATED (gitignored): uint8_*, f32_512, real_lola_512
└── benchmark-results/         # GENERATED (gitignored): <lang>.json + cli.json + summary.{md,csv}
```

`fixtures/` and `benchmark-results/` are gitignored; every run regenerates them.

## Metrics

Each language script reports the same four (plus Rust-specific metadata-open);
with `--real` an extra read of a genuine NASA LOLA elevation crop and with
`--nac` a read of the full ~55 MB NASA LRO-NAC DTM are added, and the CLI job
reports fresh-subprocess `ptiff info` timings:

| metric                           | what it times                                                |
|----------------------------------|-------------------------------------------------------------|
| `write_all_tiles_ms`             | create a 128x128 UInt8 tiled TIFF (4x64x64 tiles), write all tiles |
| `read_uint8_128_ms`              | open `fixtures/uint8_128.tif`, read **all** tiles           |
| `read_uint8_512_ms`              | open `fixtures/uint8_512.tif`, read **all** tiles           |
| `read_f32_512_ms`                | open `fixtures/f32_512.tif`, read **all** tiles (2x bytes)  |
| `read_real_lola_512_ms`          | `--real` only: read **all** tiles of a real NASA LOLA elevation crop |
| `read_nac_ms` (a.k.a. `read_nac_616`) | `--nac` only: read all 616 tiles of the real ~55 MB NASA LRO-NAC DTM |
| `read_metadata_*_ms` (Rust only) | `ptiff::open_path` metadata-only open (no per-tile read exists in the Rust binding) |
| `cli_info_uint8_128_ms`          | CLI only: fresh subprocess `ptiff info fixtures/uint8_128.tif` |
| `cli_info_nac_ms`                | CLI only: fresh subprocess `ptiff info` on the ~55 MB real LRO-NAC pyramid |
| `cli_copy_nac_ms`                | CLI only: **full end-to-end pixel copy** of the real LRO-NAC DTM via `ptiff copy` (fresh subprocess opens the read side, reads all 616 tiles, writes a throwaway copy) |

For every *pixel-read* metric (the `read_*_ms` rows) the summary prints a
companion `<metric> MB/s` row giving the **decoded pixel-data throughput**
(payload bytes / median time, in MB/s). `read_metadata_*` and `cli_info_*` have
no throughput because they read no per-tile pixels; `cli_copy_nac` reflects
read+write (see Notes).

Every timed sample repeats the tile body `BENCH_ITERS` (default **50**) times to
amortize noise; each metric is the **median** of `BENCH_REPEATS` (default **20**)
samples (same methodology as `scripts/interop_check.py`: sort → middle). The CLI
metrics have **no inner iteration** — the unit being timed *is* the subprocess
(spawn → library load → open/parse → print → exit) — so only the outer median
applies; `BENCH_ITERS` does not affect them.

Fixtures are written deterministically via the Python binding itself:
UInt8 uses the `(x·3+y·5) % 256` gradient (identical formula to the interop
fixtures), Float32 a normalized ramp. `--real` copies
`benchmarks/data/lola_real_crop_512.tif` (gitignored; same real data as
`documents/paper/ptiff/experiment/data/`, copied here so this suite runs
standalone) into `fixtures/real_lola_512.tif` and `--nac` copies
`benchmarks/data/NAC_DTM_ATLAS2.PYR.TIF` into
`fixtures/nac_dtm.tif`, so every language reads the identical real-data byte
stream. The NAC metric has a dedicated inner-iteration knob `BENCH_NAC_ITERS`
(default **2** full passes over the 616 tiles): one pass is already a large
sample, so a small multiplier keeps the ~55 MB read affordable.

## Usage

From `benchmarks/`:

```bash
./run_benchmarks.sh                 # full suite over all six languages
./run_benchmarks.sh python ruby     # subset
./run_benchmarks.sh --real          # also read a real NASA LOLA crop (needs benchmarks/data/)
./run_benchmarks.sh --nac           # also read the full real NASA NAC DTM (55 MB)
BENCH_REPEATS=10 BENCH_ITERS=25 ./run_benchmarks.sh   # shorter/faster run
```

The script needs the language bindings built against the Rust-built C ABI
(`cargo build -p ptiff-c --release` → `target/release/libptiff_c.*`): the SWIG
bindings via `make -C bindings/swig go|ruby`, the Octave MEX binding via
`make -C bindings/octave/mex build`. The Julia binding needs no adapter
compile step (Ptiff.jl calls `libptiff_c` directly via `ccall`, resolving
`target/release/` by default). The `cli` step builds the CLI
(`crates/ptiff-cli`) automatically if needed. No pkg-config or installed prefix
is used — the SWIG Go `cgo_flags.go`, the rpaths and the Rust workspace resolve
`libptiff_c` / `ptiff` from `target/release` / the cargo workspace directly.

`--real` / `--nac` require the real samples to be present in `benchmarks/data/`
(gitignored; same real data as `documents/paper/ptiff/experiment/data/`, see
`documents/paper/ptiff/experiment/README.md`). Without them the corresponding
`read_real_lola_512` / `read_nac_ms` / `cli_info_nac` / `cli_copy_nac` rows are
simply skipped, so both flags degrade gracefully (they never fail the run).

Output lands in `benchmarks/benchmark-results/`:

* `<language>.json` — raw per-metric stats (median/min/max ms, repeats, iters)
* `cli.json` — ptiff CLI end-to-end stats (full subprocess lifetime)
* `summary.md` / `summary.csv` — one row per metric, one column per language

## CI

`.github/workflows/benchmarks.yml` runs the suite on every push/PR (ubuntu
latest, Release build) with reduced `BENCH_REPEATS=5` / `BENCH_ITERS=10` as a
coarse regression check — an artifact holding the per-language JSONs and the
summary table is uploaded for diffing across PRs/cycles. For authoritative
numbers, run locally on a quiet machine with the default knobs (as above).

## Notes & interpretation

* **Rust has no per-tile read** in the binding, so its "read" rows are
  metadata-opens (`ptiff::open_path`) — flagged in the summary, not pixel
  decodes. Its `write_all_tiles_ms` is comparable to the other languages'.
* **Octave is consistently the slowest on tile I/O** (roughly 2× on
  `read_uint8_512`, ~2.2× on `read_f32_512`): each MEX call crosses the Octave
  interpreter / `.oct` boundary, which is more expensive than CPython/CRuby/cgo
  for a hot tile loop. Go and Rust are the fastest on write; Go is marginally
  the fastest on tile reads here.
* The **CLI rows** are an order of magnitude larger than the in-process rows
  (≈7 ms vs <1 ms): they measure full process *lifetime* — spawn, dylib load,
  parse, print — which dominates any single `ptiff info`. The `nac` vs `uint8`
  delta shows how much a large real-world IFD parse adds on top of startup.
* **`cli_copy_nac_ms`** is the CLI's *pixel-read* path: it drives a fresh
  `ptiff copy` subprocess that opens the read side (`ptiff::Source`), reads all
  616 tiles of the real LRO-NAC DTM and writes a throwaway copy. Because the
  whole process (spawn + read + write + close) is the unit, it lands well above
  the in-process `read_nac_ms` numbers — it is an *end-to-end* number, not a
  pure read. It requires the NAC sample and degrades to `n/a` without it.
* Absolute numbers depend on the machine and `BENCH_*` settings; the value is
  in the *relative* ordering between bindings on identical work, and in the
  trend over time when you re-run after changes.

```text
# representative run (macOS arm64, libptiff 1.0.0, repeats=20, iters=50)
| metric            | python | ruby |  go  | octave | rust  |
|-------------------|--------|------|------|--------|-------|
| write_all_tiles   | 0.401  | 0.384| 0.356| 0.662  | 0.251 |
| read_uint8_128    | 0.366  | 0.385| 0.361| 1.313  | n/a   |
| read_uint8_512    | 3.521  | 3.597| 3.404| 7.573  | n/a   |
| read_f32_512      | 10.014 |10.240| 9.934|22.102  | n/a   |
```

---

## Parallel scaling benchmark (`parallel_bench/`)

A dedicated **Phase-5** scaling micro-benchmark over the current `ptiff-core`
(with the `parallel` Rayon feature), measuring how the parallel TIFF tile
compression / decompression paths scale with worker-thread count — directly
coding hypotheses **H7** (parallel tile-decompression scales sub-linear to
linear) and **H8** (parallel compression ≥ sequential at ≥2 cores) from
`PTIFF-1.0-RUST-CORE-PLAN.md` §12.

It builds a deterministic **4096×2048 / 32768-tile** tiled TIFF in memory and
times the CPU-bound parallel paths inside explicit `rayon::ThreadPool`s of
**1/2/4/8** threads (median of 20 samples, 3 warm-ups). I/O stays sequential
by design (§6.2); only compression/decompression is parallel.

```bash
cd benchmarks/parallel_bench
cargo run --release -- --out /tmp/ptiff_parallel_bench.json
```

Each metric row reports `median_ms` and `speedup_vs_1_thread`; a representative
macOS-arm64 run (the single-thread baseline is 1.0×):

| metric          | 2 threads | 4 threads | 8 threads |
|-----------------|-----------|-----------|-----------|
| `lzw_compress`  | 1.9×      | 3.7×      | **6.9×**  |
| `lzw_decompress`| 1.9×      | 3.7×      | **7.0×**  |
| `deflate_compress` | 1.7×   | 1.8×      | 1.1×      |

LZW (hand-written codec) scales **near-linearly** — a clear H7/H8 win. Deflate
(via `flate2`/miniz_oxide) peaks at 2–4 threads (~1.8×) then degrades at 8
(more pool + memory-bandwidth overhead than the already-efficient zlib encoder
can amortise), an expected, realistic behaviour. The single-thread absolute
times are deterministic for the same host; compare *speedups* across hosts.

The crate is `exclude`d from the Cargo workspace (like `rust_bench`) so the
dependency-light `ptiff-core` default build is unaffected; it pulls `ptiff-core`
itself with `tiff-backend + tiff-codecs + parallel`.

### Idiomatic facade API (`ptiff` crate)

The parallel path is also exposed through the high-level `ptiff` crate (feature
`parallel`, forwarding `ptiff-core/parallel`):

```rust
use ptiff::Tiff;

let tiff = Tiff::open("image.ptiff")?;
let raster = tiff.read_image_pixels_parallel(0)?;   // parallel decompress (byte-identical to read_image_pixels)
let tiles  = tiff.read_all_tiles_parallel(0)?;      // one owned Vec per tile, row-major

let bytes = Tiff::to_bytes_with_pixels_parallel(&scene, &[raster.as_slice()])?; // parallel compress
```

Parallel output is byte-identical to the sequential APIs; the shared write
kernel supports single-image tiled + compressed scenes (multi-image
tiled + compressed is not combinable, matching the core).
