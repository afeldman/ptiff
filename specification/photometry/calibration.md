# PTIFF Specification — Photometry Domain

**Status:** Draft — conceptual; no dedicated private tag allocated yet
**Storage notation:** future `ptiff.photometry.*` (concept); today surfaced via
`ptiff.layers.*` on the Scientific-Layers tag 65004
**Normative basis:** `RFC-0001-Core.md` (§2, §9, §12, §17), reference implementation
`libptiff/src/io/backend/tiff/`.

---

## 1. Purpose

The Photometry domain covers the **radiometric / photometric calibration** of a planetary
image: how raw instrument counts convert to physical radiance or reflectance, the photometric
parameters used for correction (e.g. phase angle, photometric function), and any
calibration-related layer products (e.g. an albedo layer). RFC-0001 repeatedly lists
"calibration and photometric parameters" as part of the scientific context PTIFF must carry
(§3, §15) and "photometry" as one of the extension domains (§12, §17).

---

## 2. Status and current implementation

Today the reference implementation does **not** yet define a normatively fixed
`ptiff.photometry.*` field schema, and no private tag is reserved exclusively for
photometry. Concretely:

- **Radiometric calibration** is carried today as generic scientific layers
  (`ptiff.layers.*`, private tag 65004), e.g. an `albedo` map:
  `ptiff.layers.albedo = albedo_16x16`.
- A dedicated `ptiff.photometry.*` field group MAY be added in a future extension RFC,
  following the field-registration process in `appendices/field-register.md`.

Until that RFC lands, this domain is **conceptual**: the tag 65004 mechanism already lets a
producer carry photometric layer products, and the per-domain record rules of
`core/container-encoding.md` guarantee any future `ptiff.photometry.*` keys survive a
read-modify-write round trip.

---

## 3. Planned scope (reserved for a future extension RFC)

When a Photometry extension RFC is written it SHOULD specify (per RFC-0001 §17 principles):

- the canonical unit system for radiance / reflectance (see §17 items 3, 14),
- photometric correction parameters (phase angle, incidence/emission angles, photometric
  function),
- calibration history / provenance linkage (see Provenance domain and tag 65005),
- how photometric layer products map onto the Scientific-Layers tag (65004).

---

## 4. Conformance

There are currently no normative photometry requirements beyond the generic container rules of
`core/container-encoding.md`. A PTIFF reader MUST tolerate a file with no photometry metadata.

---

## 5. References

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65004 payload encoding
  and record rules.
- [`../appendices/field-register.md`](../appendices/field-register.md) — field registration.
- `RFC-0001-Core.md` — §2, §3, §9, §12, §15, §17.
