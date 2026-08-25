# RFC-0011: Compression

**Status:** Draft — conceptual / reserved, with the §20 codec decisions locked
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.2.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Compression domain scope per RFC-0001 §17 item 9 and captures the schemes already implemented by the reference implementation. |
| 0.2.0   | 2026-08-25 | PTIFF Maintainers | **PTIFF-1.0 decisions (plan §20):** ZSTD is *not* a TIFF baseline codec for 1.0 (remains Zarr-only; no standard TIFF tag value is in scope §6); the JPEG path is pinned to the deterministic pure-Rust `jpeg-encoder` / `jpeg-decoder` pair with the documented pixel-tolerance golden rule (libjpeg-turbo stays an optional, Post-1.0 parity / performance path, §6). |

---

## 1. Abstract

This RFC reserves the **Compression domain** of PTIFF: which compression schemes are permitted
for baseline and extension data, and any planetary-imagery-specific compression considerations
(e.g. high dynamic range, scientific losslessness). It resolves RFC-0001 §17 item 9.

This RFC is currently **conceptual / reserved**: it records the required scope and reflects the
compression schemes already shipped by the reference implementation. The PTIFF-1.0 codec
decisions from plan §20 are locked in §6 below; a fully normative revision (canonical per-scheme
encoding + conformance rules) remains a future extension.

---

## 2. Status and reference-implementation coverage

The reference implementation (Rust workspace, `TiffBackend`) already reads and writes these
baseline TIFF compression schemes:

| Scheme | TIFF Compression value | Predictor support |
|--------|------------------------|-------------------|
| PackBits | 32773 | — |
| LZW | 5 | horizontal-differencing (Predictor 2) on strip writes |
| Deflate | 8 (and 32946) | horizontal-differencing (Predictor 2) on strip writes |
| JPEG | 7 | pure-Rust `jpeg-encoder`/`jpeg-decoder`; grayscale and RGB/YCbCr, UInt8-only; not combined with tiled write or a predictor (tiled JPEG reads back within a documented lossy tolerance) |

The `predictor` core field (`None`, `Horizontal` (1), `FloatingPoint` (2)) is exposed on the
storage model per `specification/core/container-encoding.md`.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Compression RFC SHOULD specify:

- a canonical table of permitted compression schemes and their TIFF tag values, including
  baseline and extension data,
- **scientific losslessness** requirements for high-dynamic-range and calibrated data,
- the exact `predictor` rules and their interaction with each codec (tiled vs. stripped,
  bit depth),
- conformance rounds for each scheme in the validation suite (`conformance/`, RFC-0013).

---

## 4. Conformance

There are currently no normative compression requirements beyond the baseline TIFF rules and
the reference implementation's documented behavior. A PTIFF reader MUST tolerate any
compression scheme it cannot decode by reporting a clear decoding error rather than corrupting
output.

---

## 5. References

- `RFC-0001-Core.md` — §12 (extensibility), §17 item 9.
- `RFC-7002` — payload codec.
- Reference implementation: `crates/ptiff-core/src/io/backend/tiff/`
  (Rust; `image_source.rs`, `image_sink.rs`), `specification/core/container-encoding.md` §5.

---

## 6. PTIFF-1.0 codec decisions (plan §20)

These decisions resolve the two §20 "open questions" that touch the Compression domain. They
are binding for PTIFF 1.0 and are locked here so the RFC and the plan do not diverge.

### 6.1 ZSTD — not a TIFF baseline codec for 1.0 (Zarr-only)

**Decision:** ZSTD is **not** elevated to a TIFF baseline compression scheme for PTIFF 1.0.
`CompressionKind` remains additive with `None / Lzw / Deflate / Jpeg` only; `resolve_compression`
(`directory.rs`) continues to accept TIFF Compression values 1/5/7/8/32946/32773 and reject any
other value with a clear decoding error (RFC-0011 §4).

**Rationale:**
- **No standard TIFF tag value.** The de-facto value `34925` (Zstandard) is a non-standard,
  .NET-specific extension, not a TIFF/BigTIFF baseline. Lifting it would make PTIFF emit
  files that third-party TIFF readers are not required to understand.
- **Dependency-light / platform goals (§3.1.2).** `zstd` is intentionally an *optional*
  dependency pulled in only by the `zarr-backend` feature. Adding a TIFF-ZSTD codec would fold
  it into `tiff-codecs`, breaking the "off by default, core stays dependency-free" gate.
- **Zarr keeps ZSTD where it is wanted.** Zarr chunk codecs are per-chunk and already use
  `zstd::bulk::compress` (`backend/zarr/codec.rs`), matching the reference's `ZSTD_compressBound`
  slot geometry; that remains the sanctioned ZSTD use site.

**Consequence:** ZSTD-as-a-TIFF-codec stays a documented *Post-1.0* candidate (§7002 / plan §5.2
"ZSTD als TIFF-Codec offen"); revisiting it requires a normative RFC revision that first defines
the tag value and conformance rules, not a code-only change.

### 6.2 JPEG — pinned to the deterministic pure-Rust encoder/decoder

**Decision:** For PTIFF 1.0 the JPEG path is pinned to the **pure-Rust `jpeg-encoder` /
`jpeg-decoder`** pair already shipped in Phase 4 (`compression/jpeg.rs`, behind `tiff-codecs`),
with:
- a fixed baseline-4:4:4 encoding (no chroma subsampling, `SamplingFactor::F_1_1`),
- UInt8-only, `samplesPerPixel` 1 (grayscale) or 3 (RGB),
- golden validation by **pixel tolerance** (e.g. `max_abs_diff`), *not* byte-exact comparison
  (RFC-0011 §2 / plan §11.3), because JPEG is lossy.

**Rationale:**
- **Determinism (§14).** A free-standing pure-Rust codec has no dependence on a system
  `libjpeg`/CPU-SIMD build, so the same input + quality always yields identical bytes and
  identical decoded pixels. It *improves* cross-build reproducibility over libjpeg-turbo's
  version/CPU-sensitivity.
- **Dependency-light / platform (§3.1.2).** No system toolchain; the core stays a pure-Cargo
  build with a small, pure-Rust dependency surface (`jpeg-encoder` 0.7, `jpeg-decoder` 0.3).
- **Consistency with the shipped implementation.** Phase 4 already implemented JPEG this way;
  this decision does not force a rework at 1.0 and keeps the golden/tolerance suite as-is.

**libjpeg-turbo remains an optional, Post-1.0 path** for throughput/SIMD parity with the C++
oracle (plan §5.3 "erste Wahl" for performance). It is limited to a **bridge-only, feature-gated
path behind the C-ABI** (Python/Octave/Go/Ruby/C++-wrapper), per the **Pure-Rust principle**
(plan §3.1.9: *core & Rust interfaces are pure-Rust; bridges may use native libraries*); it is
never a Rust-interface/kern dependency. If/when introduced, it must document its pinned minimum
version and coexist with the pure-Rust default so the deterministic default path is unchanged.
