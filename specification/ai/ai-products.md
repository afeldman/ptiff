# PTIFF Specification — AI / ML Domain

**Status:** Draft — conceptual; no dedicated private tag allocated yet
**Storage notation:** future `ptiff.ai.*` (concept)
**Normative basis:** `RFC-0001-Core.md` (§9 UC5, §17 item 7).

---

## 1. Purpose

The AI domain represents **AI / ML-derived products** attached to a planetary image:
segmentation, classification, uncertainty maps, and embeddings, together with the **provenance**
of those products (model identity, version). RFC-0001's use case:

> **UC5 — AI-derived product overlay.** A machine learning pipeline attaches classification,
> segmentation, or uncertainty maps as derived layers or companion products. — RFC-0001 §9

---

## 2. Status and current implementation

The AI domain is **conceptual / reserved**: no dedicated private tag is currently allocated
and no `ptiff.ai.*` field schema is yet defined. RFC-0001 §17 item 7 states the required
scope:

> **AI layer** — representation of AI/ML-derived products (segmentation, classification,
> uncertainty, embeddings) and their provenance (model identity, version). — RFC-0001 §17 item 7

Two existing mechanisms already cover part of the operational need today:

- **Per-pixel AI products** (segmentation/classification/confidence maps) are per-pixel
  derived layers and can be carried on the **Scientific-Layers tag (65004)** using
  `ptiff.layers.*` keys (e.g. `ptiff.layers.confidence`).
- **Process provenance** — including the software / model that produced a product — belongs
  on the **Provenance tag (65005)**, `ptiff.provenance.*` (e.g. `software`, `operator`,
  `commit`).

A future AI RFC will define a dedicated `ptiff.ai.<product>.*` schema and link AI-product
provenance (model id, version, training data reference) to the Provenance domain.

---

## 3. Planned scope (reserved for a future extension RFC)

When the AI extension RFC is written it SHOULD specify:

- the `ptiff.ai.*` product vocabulary (segmentation, classification, uncertainty, embeddings),
- each product's **dtype, class/taxonomy reference, and coordinate alignment** to the baseline
  image,
- **AI-provenance** (model identity, version, traceability) and its linkage to the Provenance
  domain (tag 65005),
- how AI layer products map onto the Scientific-Layers tag (65004).

---

## 4. Conformance

There are currently no normative AI requirements beyond the generic container rules. A PTIFF
reader MUST tolerate a file with no AI metadata.

---

## 5. References

- [`../core/container-encoding.md`](../core/container-encoding.md) — tags 65004/65005 record rules.
- [`../appendices/field-register.md`](../appendices/field-register.md) — field registration.
- `RFC-0001-Core.md` — §9 UC5, §17 item 7.
