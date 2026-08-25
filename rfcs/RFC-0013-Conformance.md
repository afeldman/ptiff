# RFC-0013: Validation & Conformance Testing

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-0011 (Compression), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.2.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Validation & Conformance domain scope per RFC-0001 §17 item 11 and reflects the conformance-level ladder already defined in `conformance/levels.md`. |
| 0.2.0   | 2026-08-25 | PTIFF Maintainers | Records that the reserved **Post-1.0 paths** (RFC-0016 ZSTD-as-TIFF-codec, RFC-0017 libjpeg-turbo bridge path) bring their **own conformance rounds** and land in the ladder only when/if adopted — they are never folded into the 1.0 core suite (see §2/§3). |

---

## 1. Abstract

This RFC reserves the **Validation & Conformance** domain of PTIFF: the structure of the
conformance test suite (`conformance/`), conformance levels, and the certification /
self-declaration process by which an implementation may claim PTIFF conformance. It resolves
RFC-0001 §17 item 11.

A conformance suite is the executable + documentary agreement by which an implementation may
claim to be PTIFF-conformant: what is tested, how deeply (a ladder of levels), and the evidence
a self-declaration must cite.

---

## 2. Status and existing conformance ladder

The reference conformance suite (`conformance/`) already defines a **cumulative level ladder**
in `conformance/levels.md`:

- **Level 0 — Baseline / container:** a PTIFF-unaware generic TIFF/BigTIFF reader can read the
  file; correct byte-order, IFD integrity, baseline image decode.
- **Level 1 — PTIFF Core:** PTIFF structured metadata encoded via private tags without breaking
  generic readability; round-trips the structured metadata. Not yet ratified — blocked on the
  tag-allocation RFC (RFC-7002).

Higher levels (camera, CRS, SPICE, stereo, photometry, mesh, AI) are reserved for the
corresponding extension RFCs and land in the suite as those RFCs are ratified.

**Post-1.0 paths bring their own conformance rounds.** The two reserved Post-1.0 RFCs —
**RFC-0016** (ZSTD as a TIFF compression scheme) and **RFC-0017** (libjpeg-turbo bridge path) —
are explicitly **not** part of the PTIFF 1.0 core suite. If/when adopted, each contributes its
own conformance rounds and a corresponding ladder location:

- **RFC-0016 §3.5 / §5** — ZSTD conformance rounds (window boundaries, predictor interaction,
  lossless byte round-trip) and the ladder level at which ZSTD conformance is claimed; until
  adopted, a reader MUST still report a clear decoding error for out-of-scope `Compression`
  values (RFC-0011 §4).
- **RFC-0017 §6 / §7** — bridge-path tests are **feature-gated and bridge-side**, proving the
  native path coexists with the pure-Rust default without disturbing it; they never enter the
  core golden suite and never change core conformance obligations.

Executable tests live in `crates/ptiff-core/tests/` (Rust integration tests), the tracing
matrix in `conformance/matrix.md`. Keywords use RFC 2119 / RFC 8174 per RFC-0001 §10.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Conformance RFC SHOULD specify:

- the conformance-level vocabulary (baseline, PTIFF core, extension domains) and their
  cumulative ordering,
- how each extension-domain RFC contributes levels to the ladder,
- the **self-declaration / certification** process and the evidence required (test-run cites),
- the mapping of executable tests to levels in the tracing matrix,
- explicitly that **Post-1.0 paths carry their own conformance rounds** with a defined ladder
  location (as scoped in RFC-0016 §3.5 and RFC-0017 §6), never folded into the 1.0 basic suite.

---

## 4. Conformance

The conformance suite itself is informative-to-normative per RFC-0001 §10 / NG5. An
implementation MAY self-declare conformance only by citing passing executable tests at the
claimed level.

---

## 5. References

- `RFC-0001-Core.md` — §10 (terminology), §17 item 11, non-goal NG5.
- `RFC-7002` — tag-allocation basis for Level 1.
- `RFC-0016-ZSTD-TIFF.md` — §3.5 / §5 (Post-1.0 ZSTD conformance rounds).
- `RFC-0017-JPEG-LibjpegTurbo-Bridge.md` — §6 / §7 (bridge-side, feature-gated tests).
- Reference implementation: `conformance/levels.md`, `conformance/matrix.md`,
  `crates/ptiff-core/tests/`.
