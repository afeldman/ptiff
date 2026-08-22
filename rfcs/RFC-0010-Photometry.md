# RFC-0010: Photometry & Radiometric Calibration

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
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Photometry domain scope per RFC-0001 §2, §9, §12, §15. No dedicated private tag is allocated yet. |

---

## 1. Abstract

This RFC reserves the **Photometry domain** of PTIFF — the radiometric / photometric
calibration of a planetary image: how raw instrument counts convert to physical radiance or
reflectance, the photometric parameters used for correction (phase angle, photometric
function), and calibration-related layer products (e.g. an albedo layer).

RFC-0001 lists "calibration and photometric parameters" as part of the scientific context PTIFF
MUST carry (§3, §15) and names photometry as one of the extension domains (§12). There is no
dedicated RFC-0001 §17 item for photometry; this RFC is the dedicated domain document.

---

## 2. Status and current implementation

The reference implementation does **not** yet define a normatively fixed `ptiff.photometry.*`
field schema, and no private tag is reserved exclusively for photometry. Today:

- **Radiometric calibration** is carried as generic scientific layers (`ptiff.layers.*`, private
  tag 65004), e.g. an albedo map: `ptiff.layers.albedo = albedo_16x16`.
- A dedicated `ptiff.photometry.*` field group MAY be added in a future extension RFC following
  the field-registration process in `specification/appendices/field-register.md`.

Until that RFC lands, this domain is conceptual: the tag-65004 mechanism already lets a producer
carry photometric layer products, and the record rules of RFC-7002 guarantee any future
`ptiff.photometry.*` keys survive a read–modify–write round trip.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Photometry RFC SHOULD specify:

- the canonical unit system for radiance / reflectance (see RFC-0001 §17 items 3, 14),
- photometric correction parameters (phase angle, incidence / emission angles, photometric
  function),
- calibration history / provenance linkage (see Provenance domain, RFC-0009),
- how photometric layer products map onto the Scientific-Layers tag (65004).

---

## 4. Conformance

There are currently no normative photometry requirements beyond the generic container rules of
RFC-7002. A PTIFF reader MUST tolerate a file with no photometry metadata.

---

## 5. References

- `RFC-0001-Core.md` — §2, §3, §9, §12, §15, §17 items 3, 14.
- `RFC-0009` — Provenance domain (calibration history linkage).
- `RFC-7002` — payload codec, tag 65004.
- Reference implementation: `specification/photometry/calibration.md`,
  `specification/appendices/field-register.md`.
