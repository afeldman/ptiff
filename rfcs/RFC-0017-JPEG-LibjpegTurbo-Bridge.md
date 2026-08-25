# RFC-0017: libjpeg-turbo Bridge Path (Post-1.0)

**Status:** Draft — Post-1.0 (reserved; NOT active for PTIFF 1.0)
**Category:** Non-normative for 1.0 — implementation/parity guidance for the language bridges
**Requires:** RFC-0001 (PTIFF Core), RFC-0011 (Compression), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-25

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-25 | PTIFF Maintainers | Initial draft. Reserves the Post-1.0 libjpeg-turbo path available to the language bridges behind the C-ABI, per plan §20/Q2 and RFC-0011 §6.2. NOT normative for 1.0: the deterministic pure-Rust `jpeg-encoder`/`jpeg-decoder` pair remains the only JPEG path in the Rust core and interfaces. |

---

## 1. Abstract

This RFC scopes the **Post-1.0, optional, feature-gated libjpeg-turbo path** available only to
the **language bridges** (Python/Octave/Go/Ruby/C++-wrapper) behind the **C-ABI**, for SIMD/
throughput parity with the C++ oracle. For PTIFF 1.0 the JPEG path is locked to the deterministic
pure-Rust `jpeg-encoder`/`jpeg-decoder` pair (plan §20/Q2, RFC-0011 §6.2); libjpeg-turbo is
**not** a core or Rust-interface dependency.

This document has **no normative force for 1.0**. It exists so a future bridge-performance
effort has a defined, consistent, and `determinism-safe` integration contract that never leaks a
native dependency into the Rust interface/kern.

---

## 2. Status

Draft / reserved, **Post-1.0**. Adoption is a bridge-only feature behind the C-ABI; it must not
change the 1.0 pure-Rust default path. Until adopted:

- the Rust core and all Rust interfaces use the pure-Rust pair only (`compression/jpeg.rs`,
  behind `tiff-codecs`, baseline 4:4:4, pixel-tolerance golden);
- libjpeg-turbo appears nowhere in `ptiff-core` / `ptiff` / `ptiff-c` / CLI dependencies.

---

## 3. Boundary: Pure-Rust principle (plan §3.1.9)

Per the Pure-Rust principle (plan §3.1.9) the **core and Rust interfaces are pure-Rust**;
**bridges may use native libraries**. This RFC is the concrete 1.0-consistent expression of that
rule for JPEG:

- `ptiff-core`, `ptiff` facade, `ptiff-c`, and the CLI MUST remain pure-Rust (Safe Rust; minimal
  documented `unsafe` only at the FFI boundary).
- libjpeg-turbo MAY be introduced **only behind the C-ABI**, as a feature-gated, optional
  enhancement for the language bridges.
- The **deterministic pure-Rust default must remain unchanged and always available**. The
  bridge path is opt-in; it never replaces the default, never changes default output bytes.
- simd/hardware-dependent behavior is confined to the bridge (mirroring plan §14: the simd
  feature stays opt-in and non-reproducible-capable by design).

---

## 4. Where libjpeg-turbo is permitted (and where it is not)

| Concern | 1.0 (locked) | Post-1.0 bridge path |
|---------|--------------|----------------------|
| Rust core `ptiff-core` | pure-Rust pair only | **never** a dependency |
| `ptiff` facade / `ptiff-c` / CLI | pure-Rust pair only | **never** a dependency |
| Python/Octave/Go/Ruby/C++ bridges | (bridge could opt-in if desired) | **yes**, feature-gated behind C-ABI |
| Default encoded bytes / golden | pure-Rust pair (deterministic) | bytes from the bridge path are a **separate, documented** output and never the default |

---

## 5. Requirement: determinism isolation

The two paths MUST be cleanly separable so the deterministic 1.0 property (plan §14) is never
violated:

- **same input + config ⇒ identical bytes/pixels** holds for the pure-Rust default always;
- the libjpeg-turbo bridge path is an **opt-in, feature-gated** route whose output may differ
  from the pure-Rust pair (libjpeg-turbo is CPU/SIMD-version-sensitive), so its golden rule is
  pixel-tolerance, not byte-exact — and any test that asserts on it MUST be **feature-gated**
  and labeled as bridge-path, never folded into the core golden suite;
- a codec decision in the bridges that selects the native path MUST be explicit and user-visible,
  never implicit.

---

## 6. Conformance / version pinning

Adoption MUST follow these rules:

- **pin a minimum libjpeg-turbo version** and document the exact `turbojpeg` FFI / `jpeg` crate
  binding used (RFC-0011 §6.2 originally used `turbojpeg`-FFI / `jpeg`-Crate as candidate paths;
  a future revision pins one);
- keep the ABI layer limited to `ptiff-c` so the Rust interface ABI is unchanged;
- add feature-gated integration tests (bridge-side) that prove the native path **coexists** with
  the pure-Rust default without disturbing it;
- update RFC-0011 §3 (canonical per-scheme table) only if the RFC's normative per-scheme rules
  change, otherwise document the bridge path as a performance-only, non-normative alternative.

---

## 7. Conformance

No PTIFF 1.0 requirement changes. A conformant 1.0 implementation MUST decode/encode JPEG via
the pure-Rust pair; the bridge path is optional and outside core conformance. Any reference to
libjpeg-turbo in §5.3/§13.2 of the plan that dates from the C++-reference era is **historical**
(plan §4.5 mark; §5.3 Update-Statement).

---

## 8. References

- `RFC-0011-Compression.md` — §2 (JPEG), §6.2 (1.0 pure-Rust pin + Post-1.0 bridge path).
- `RFC-0001-Core.md` — §12 (extensibility), §17 item 9 (compression).
- Plan `PTIFF-1.0-RUST-CORE-PLAN.md` — §20/Q2, §3.1.9 (Pure-Rust principle), §4.5 (bridges/C-ABI),
  §5.3 (historical libjpeg-turbo references), §13.2 (build), §14 (determinism).
- Reference implementation: `crates/ptiff-core/src/io/backend/tiff/compression/jpeg.rs`
  (pure-Rust pair), `ptiff-c` (C-ABI).
