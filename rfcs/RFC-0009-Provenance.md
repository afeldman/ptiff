# RFC-0009: Provenance & Versioning Metadata

**Status:** Draft
**Category:** Normative
**Requires:** RFC-0001 (PTIFF Core), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reflects the provenance fields shipped by the reference implementation (`ptiff.provenance.*`, private tag 65005). |

---

## 1. Abstract

This RFC defines the **Provenance domain** of PTIFF — how processing history, software
versions, and reproducibility metadata are represented, and how derived products link back to
the software or model that produced them. It is carried by the private tag **65005**
(`PtiffProvenance`) under `ptiff.provenance.*`. This RFC resolves RFC-0001 §17 item 12.

Provenance is the cross-cutting record of *who/what produced this file and its derived layers*.
It underpins the scientific-integrity and reproducibility goals of RFC-0001 (§15) and is
referenced by other extension domains (AI products, photometric calibration) when they need to
record the software or model that produced a product.

---

## 2. Status

Draft. The field schema is normative and matches the reference field register
(`specification/appendices/field-register.md`).

---

## 3. Field register

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.provenance.software` | string | no | Software / library that produced the file, e.g. `libptiff-0.3.0` |
| `ptiff.provenance.operator` | string | no | Operator / pipeline / tool name, e.g. `interop-fixture-generator` |
| `ptiff.provenance.commit` | string | no | Source revision (e.g. commit hash) of the producing software |

### 3.1 Example values (from the interop fixture)

```
ptiff.provenance.software = libptiff-0.3.0
ptiff.provenance.operator = interop-fixture-generator
ptiff.provenance.commit   = f73dec27469f878a8de9d209e0c81bf33e8b8d0a
```

---

## 4. Semantics

- **`software`** names the producing software and, when applicable, its semantic version.
- **`operator`** names the pipeline, job, or person/role that ran the processing.
- **`commit`** records the exact source revision, enabling bit-level reproducibility of the
  producing toolchain.
- A reader MUST tolerate a file with no provenance metadata.
- Unknown `ptiff.provenance.*` keys MUST be preserved verbatim (RFC-7002 §4.3), so additional
  provenance fields introduced by a future writer survive a read–modify–write round trip.

---

## 5. Linkage to other domains

- **AI products (RFC-0007)** — model identity and version used to derive a product belong on
  `ptiff.provenance.*` (or a future AI-specific schema) and SHOULD link to this domain.
- **Photometry (RFC-0010)** — calibration history SHOULD link to this domain for reprocessing
  traceability.

---

## 6. Conformance

- A **writer** MUST serialize provenance fields into tag 65005 using the RFC-7002 codec.
- A **reader** MUST decode tag 65005 if present and MUST tolerate its absence.

---

## 7. References

- `RFC-0001-Core.md` — §15 (long-term vision), §17 item 12.
- `RFC-7002` — payload codec, tag 65005.
- `RFC-0007` — AI domain (product provenance linkage).
- `RFC-0010` — Photometry domain (calibration history linkage).
- Reference implementation: `specification/appendices/field-register.md` (§2.5).
