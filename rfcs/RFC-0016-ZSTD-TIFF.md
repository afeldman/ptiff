# RFC-0016: ZSTD as a TIFF Compression Scheme (Post-1.0)

**Status:** Draft — Post-1.0 (reserved; NOT active for PTIFF 1.0)
**Category:** Normative (planned, future)
**Requires:** RFC-0001 (PTIFF Core), RFC-0011 (Compression), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-25

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-25 | PTIFF Maintainers | Initial draft. Reserves the Post-1.0 path for ZSTD as a TIFF compression scheme, per plan §20/Q1 and RFC-0011 §6.1. Explicitly NOT normative for PTIFF 1.0; captures the tag-value, geometry, and conformance questions a future normative revision must resolve before any code change. |

---

## 1. Abstract

This RFC scopes and reserves the **Post-1.0 elevation of ZSTD to a TIFF baseline compression
scheme**. For PTIFF 1.0 the decision is locked as **NO** (plan §20/Q1, RFC-0011 §6.1): ZSTD
remains purely a Zarr chunk codec — `CompressionKind` is `None / Lzw / Deflate / Jpeg` only, and
TIFF `Compression` values accepted by `resolve_compression` (`directory.rs`) stay
`1 / 5 / 7 / 8 / 32946 / 32773`.

This document therefore has **no normative force for 1.0**. It exists so that revisiting ZSTD
does **not** start from scratch: it records the exact normative gaps that must be closed first,
the candidate tag value, the geometry that would be required for scientific losslessness, and
the conformance additions (RFC-0011 §3, RFC-0013) that a future revision must bring.

---

## 2. Status

Draft / reserved, **Post-1.0**. Adoption requires a future **normative RFC revision with a
version bump** (RFC-0011-style), not a code-only feature. Until that revision is accepted:

- `CompressionKind` stays `None / Lzw / Deflate / Jpeg`;
- TIFF `Compression` values other than `1/5/7/8/32946/32773` continue to be **rejected** with a
  clear decoding error (RFC-0011 §4);
- ZSTD appears only as a Zarr chunk codec (`backend/zarr/codec.rs`, via the `zarr-backend`
  feature).

---

## 3. Scoping questions a future normative revision MUST resolve

The following are the normative gaps identified during the 1.0 decision. A future RFC that
elevates ZSTD to a TIFF codec must answer all of them explicitly.

### 3.1 Tag value

The de-facto value **34925 (Zstandard)** is a non-standard, .NET-specific TIFF extension, not a
TIFF/BigTIFF baseline. A future revision must decide between:

- adopting `34925` as a *documented extension value* (interop limited to readers that know it),
  or
- defining PTIFF's own tag allocation in a private range (via RFC-7002-style allocation), which
  sacrifices third-party interop but avoids the .NET collision.

The RFC **must** recommend one, give its `Compression` field type (`SHORT`), the required
TIFF `Predictor` interaction, and the exact encoder/decoder byte geometry.

### 3.2 Block geometry and window sizing

ZSTD is a blocking codec. A TIFF integration needs a defined mapping to TIFF strips/tiles:

- fixed **window / block size** (e.g. 64 KiB–1 MiB) with a stable, deterministic boundary rule;
- handling of strips/tiles whose byte length is not a multiple of the window — whether and how
  they are padded, and how a reader detects the logical end (zstd frame content-size vs. TIFF
  strip byte count);
- the `Compression`
  predictor interaction (horizontal-differencing for UInt8/FloatingPoint, mirroring the
  LZW/Deflate predictor rules in RFC-0011 §3).

### 3.3 Scientific losslessness (HDR / calibrated data)

PTIFF targets scientific, high-dynamic-range and calibrated planetary data. The RFC must specify:

- **scientific losslessness** requirements for ZSTD-compressed data (no lossy transform; lossless
  byte round-trip),
- whether Predictor 2 (FloatingPoint) is mandatory for HDR/reduced-range float without lossy
  surprises, and
- golden-validation mode: byte-exact (unlike the lossy-JPEG pixel-tolerance rule of RFC-0011
  §6.2) — ZSTD is lossless, so the 1.0 rule for lossy codecs does **not** apply here.

### 3.4 Dependency policy (Pure-Rust principle, plan §3.1.9)

The `zstd` crate binds `libzstd`. Bringing ZSTD into `tiff-codecs` would fold a C dependency into
the core, which is exactly what the Pure-Rust principle (§3.1.9) forbids for the core and Rust
interfaces. A future revision must specify how a ZSTD-TIFF codec can be added **without** a
native mandatory dependency in `ptiff-core`:

- a **pure-Rust ZSTD** implementation (e.g. `ruzstd`) as the core default, gated behind a
  feature and providing byte-deterministic output, or
- if raw-simd throughput parity with `libzstd` is ever required, a **bridge-only, feature-gated
  path behind the C-ABI** (mirroring the libjpeg-turbo rule, RFC-0011 §6.2) — never in the Rust
  interface/kern.

### 3.5 Interop and conformance

Contributions to RFC-0011 §3 (canonical per-scheme table) and RFC-0013 (conformance rounds):
- a canonical table row for the ZSTD tag value + predictor rules;
- conformance tests for window boundaries, predictor interaction, and lossless byte round-trip;
- the level-ladder location (RFC-0013 `conformance/levels.md`) at which ZSTD conformance is
  claimed.

---

## 4. Interplay with Zarr

ZSTD remains fully supported in the Zarr path independent of this RFC. The Zarr chunk codec
(`backend/zarr/codec.rs`, `zstd::bulk::compress`) is the sanctioned 1.0 ZSTD use site and is
unchanged. Nothing in this RFC alters Zarr behavior; it only concerns a *future* TIFF
integration that shares the ZSTD algorithm behind a different, TIFF-scoped interface.

---

## 5. Conformance

Until adopted, no PTIFF reader is required to understand TIFF `Compression` 34925 (or any
PTIFF-private ZSTD value); per RFC-0011 §4 it MUST report a clear decoding error rather than
corrupt output. Once adopted via a future normative revision, the new table row + conformance
rounds become binding.

---

## 6. References

- `RFC-0011-Compression.md` — §3 (planned scope), §5 (requirements), §6.1 (1.0 NO decision).
- `RFC-0001-Core.md` — §12 (extensibility), §17 item 9 (compression).
- `RFC-0013-Conformance.md` — conformance rounds and level ladder.
- Plan `PTIFF-1.0-RUST-CORE-PLAN.md` — §20/Q1, §5.2 (ZSTD `zstd` crate), §3.1.9 (Pure-Rust
  principle), §3.1.2 (dependency-light).
- Reference implementation: `crates/ptiff-core/src/io/backend/tiff/` (`directory.rs`), Zarr
  codec `crates/ptiff-core/src/io/backend/zarr/codec.rs`.
