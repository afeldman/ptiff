# RFC-0013: Validation & Conformance Testing

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-0011 (Compression), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Validation & Conformance domain scope per RFC-0001 §17 item 11 and reflects the conformance-level ladder already defined in `conformance/levels.md`. |

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

Executable tests live in `libptiff/tests/conformance/`, the tracing matrix in
`conformance/matrix.md`. Keywords use RFC 2119 / RFC 8174 per RFC-0001 §10.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Conformance RFC SHOULD specify:

- the conformance-level vocabulary (baseline, PTIFF core, extension domains) and their
  cumulative ordering,
- how each extension-domain RFC contributes levels to the ladder,
- the **self-declaration / certification** process and the evidence required (test-run cites),
- the mapping of executable tests to levels in the tracing matrix.

---

## 4. Conformance

The conformance suite itself is informative-to-normative per RFC-0001 §10 / NG5. An
implementation MAY self-declare conformance only by citing passing executable tests at the
claimed level.

---

## 5. References

- `RFC-0001-Core.md` — §10 (terminology), §17 item 11, non-goal NG5.
- `RFC-7002` — tag-allocation basis for Level 1.
- Reference implementation: `conformance/levels.md`, `conformance/matrix.md`,
  `libptiff/tests/conformance/`.
