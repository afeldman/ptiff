# RFC-0007: AI / ML Layer Products

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-0009 (Provenance), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the AI-domain scope per RFC-0001 §17 item 7; no dedicated `ptiff.ai.*` schema is defined yet. |

---

## 1. Abstract

This RFC reserves the **AI domain** of PTIFF: AI/ML-derived products attached to a planetary
image — segmentation, classification, uncertainty maps, embeddings — together with the
provenance of those products (model identity, version). RFC-0001's use case:

> **UC5 — AI-derived product overlay.** A machine learning pipeline attaches classification,
> segmentation, or uncertainty maps as derived layers or companion products.
> — RFC-0001 §9

This RFC is currently **conceptual / reserved**: no dedicated private tag is allocated and no
`ptiff.ai.*` field schema is defined yet. It records the required scope, resolving RFC-0001
§17 item 7.

---

## 2. Status and existing mechanisms

Two existing mechanisms already cover part of the operational need:

- **Per-pixel AI products** (segmentation / classification / confidence maps) are per-pixel
  derived layers and can be carried on the **Scientific-Layers tag (65004)** using
  `ptiff.layers.*` keys (e.g. `ptiff.layers.confidence`).
- **Process provenance** — the software / model that produced a product — belongs on the
  **Provenance tag (65005)**, `ptiff.provenance.*` (e.g. `software`, `operator`, `commit`).

A future AI RFC will define a dedicated `ptiff.ai.<product>.*` schema and link AI-product
provenance (model id, version, training-data reference) to the Provenance domain.

---

## 3. Planned scope (required of a future extension RFC)

A future normative AI RFC SHOULD specify:

- the `ptiff.ai.*` product vocabulary (segmentation, classification, uncertainty, embeddings),
- each product's **dtype, class / taxonomy reference, and coordinate alignment** to the baseline
  image,
- **AI-provenance** (model identity, version, traceability) and its linkage to the Provenance
  domain (RFC-0009, tag 65005),
- how AI layer products map onto the Scientific-Layers tag (65004).

---

## 4. Conformance

There are currently no normative AI requirements beyond the generic container rules of RFC-7002.
A PTIFF reader MUST tolerate a file with no AI metadata.

---

## 5. References

- `RFC-0001-Core.md` — §9 UC5, §17 item 7.
- `RFC-0009` — Provenance domain (product provenance linkage).
- `RFC-7002` — payload codec, tags 65004 / 65005.
- Reference implementation: `specification/ai/ai-products.md`,
  `specification/appendices/field-register.md`.
