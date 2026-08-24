# PTIFF Conformance Matrix

The matrix maps every normative requirement in [levels.md](levels.md) to the executable tests
that prove it, and marks coverage status. The "Green" (implemented) rows are the first
executable block of this suite: **Level 0 baseline/container conformance**.

Legend for status:

- ✅ implemented & wired into CTest
- 🚧 requirement defined, tests not yet written
- ⛔ blocked on a missing RFC / dependency

## Level 0 — Baseline / container

| Req | Requirement | Executable test(s) | Status |
|-----|-------------|--------------------|--------|
| L0.1 | File parses as valid TIFF / BigTIFF | `baseline_tiff_conformance_test.cpp` | ✅ |
| L0.2 | Byte order + magic (`II`/`MM`, 42/43) | `` "recognizes little-endian classic TIFF" `` / BigTIFF cases | ✅ |
| L0.3 | First IFD offset valid; IFD chain sound | minimal-file open + multi-IFD read | ✅ |
| L0.4 | Baseline tags present & consistent | grayscale/RGB model fields validated on read | ✅ |
| L0.5 | Pixel layouts read byte-for-byte | gray/RGB 8/16/32 uint + 32 float, uncompressed & compressed | ✅ |
| L0.6 | `TiffBackend` passes `[conformance][baseline]` | whole suite | ✅ |

## Level 1 — PTIFF Core

| Req | Requirement | Executable test(s) | Status |
|-----|-------------|--------------------|--------|
| L1.1 | Level 0 satisfied | (inherits Level 0) | ✅ via Level 0 |
| L1.2 | Metadata in private/tag ranges only | `[conformance][ptiff-core]` (future) | 🚧 |
| L1.3 | Metadata round-trip without breaking L0 readability | `ptiff_metadata` round-trip (future) | 🚧 |
| L1.4 | Tag-allocation RFC | — | ⛔ blocked on RFC |

## Level 2 — Extension domains

Each domain maps to its own normative RFC + `conformance/<domain>/` + tests. All currently
`⛔` (RFC not yet written); see [levels.md](levels.md) §"Extension domain".

| Req | Domain | Executable test(s) | Status |
|-----|--------|--------------------|--------|
| L2.x | Camera | `conformance/camera/` | ⛔ |
| L2.x | CRS | `conformance/crs/` | ⛔ |
| L2.x | SPICE | `conformance/spice/` | ⛔ |
| L2.x | Stereo | `conformance/stereo/` | ⛔ |
| L2.x | Photometry | `conformance/photometry/` | ⛔ |
| L2.x | Mesh | `conformance/mesh/` | ⛔ |
| L2.x | AI | `conformance/ai/` | ⛔ |

## Level 3 — Full PTIFF

Sum of Levels 0–2; follows once the domain rows above are ratified.

---

## Test file index (`crates/ptiff-core/tests/`)

The Rust workspace enforces the baseline (Level 0) requirements via the `ptiff-core`
integration tests. The dedicated C++ conformance files that lived under
`libptiff/tests/conformance/` were superseded by these Rust tests during the
Rust migration.

| File | Covers |
|------|--------|
| `crates/ptiff-core/tests/tiled_write.rs` | Level 0: header/IFD write, tiled single-image TIFF/BigTIFF, pixel byte-for-byte round-trip |
| `crates/ptiff-core/tests/corrupted.rs` | Robustness: bad byte-order/magic, truncated data, malformed IFDs rejected with a defined `ErrorCode` |
| `crates/ptiff-core/tests/golden.rs` | Byte-exact serialized output (golden digests) |
| `crates/ptiff-core/tests/property.rs` | Property tests over tile arithmetic and lossless codecs |
