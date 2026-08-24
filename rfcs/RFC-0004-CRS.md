# RFC-0004: Planetary Coordinate Reference Systems

**Status:** Draft
**Category:** Normative
**Requires:** RFC-0001 (PTIFF Core), RFC-0002 (GeoTIFF Relationship), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reflects the planetary CRS fields shipped by the reference implementation (`ptiff.crs.*`, private tag 65003). |

---

## 1. Abstract

This RFC defines the **CRS domain** of PTIFF — how a planetary dataset is georeferenced: which
planetary body it depicts, which reference-frame realization is used, and which map projection
maps body coordinates to the image plane. PTIFF reuses established georeferencing conventions
(see RFC-0002) rather than inventing a parallel mechanism, so that PTIFF-aware tooling can
interoperate with the GeoTIFF / GIS ecosystem. It is carried by the private tag **65003**
(`PtiffCrs`) under `ptiff.crs.*`. This RFC resolves RFC-0001 §17 item 4.

> PTIFF SHOULD prefer reusing existing, well-established conventions (e.g., GeoTIFF tag
> conventions, established CRS identifiers, SPICE kernel conventions) over inventing new ones.
> — RFC-0001, §8 Design Principles

---

## 2. Status

Draft. The field schema is normative and matches the reference implementation
(`crates/ptiff-rust/examples/write_ptiff_fixture.rs`). It builds on RFC-0002's relationship to GeoTIFF.

---

## 3. Field register

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.crs.body` | string | no | Planetary body identifier, e.g. a NAIF body ID such as `301` (the Moon) |
| `ptiff.crs.projection` | string | no | Map projection name, e.g. `equirectangular` |
| `ptiff.crs.reference_frame` | string | no | Reference frame / datum realization, e.g. `IAU_MOON_2000` |

### 3.1 Example values (from the interop fixture)

```
ptiff.crs.body            = 301
ptiff.crs.projection      = equirectangular
ptiff.crs.reference_frame = IAU_MOON_2000
```

---

## 4. Semantics

- **`body`** identifies the planetary body. NAIF body IDs (`301` = the Moon, `399` = Earth,
  `499` = Mars, …) are the canonical form. Other recognized identifiers MAY be used and SHOULD
  be registered in the field-register appendix.
- **`projection`** names the map projection. The reference implementation exercises
  `equirectangular`; additional projections are future work.
- **`reference_frame`** names the frame / datum realization that ties body coordinates to
  inertial / world coordinates (e.g. a planet-fixed frame). Combined with the Camera extrinsics
  (RFC-0003), this completes the description of how image pixels map onto the planetary surface.

---

## 5. Interoperability with GeoTIFF

Per RFC-0002, PTIFF reuses GeoTIFF's CRS conventions where they apply to non-Earth bodies:

- For Earth-centric data, the established GeoTIFF tags (33550–34735) SHOULD be honored and the
  `ptiff.crs.*` fields provide the planetary-science overlay.
- Where a PTIFF extension domain overlaps in purpose with an established convention, PTIFF
  SHOULD reuse or explicitly extend that convention rather than define a parallel, incompatible
  mechanism — unless a documented planetary-science-specific limitation requires otherwise
  (RFC-0001 §11).
- A GIS tool SHOULD be able to load a PTIFF orthomosaic using planetary CRS metadata analogous
  to how GeoTIFF is used for Earth basemaps today (RFC-0001 §9, UC4).

---

## 6. Conformance

- A **writer** MUST serialize CRS fields into tag 65003 using the RFC-7002 codec.
- A **reader** MUST tolerate a file with no CRS metadata and MUST NOT require GeoTIFF tags to
  be present to read the baseline image.

---

## 7. References

- `RFC-0001-Core.md` — §8, §9 UC4, §11, §17 item 4.
- `RFC-0002` — GeoTIFF relationship / interop.
- `RFC-7002` — payload codec, tag 65003.
- `RFC-0003` — Camera domain (intrinsics/extrinsics mapping pixels to the surface).
- Reference implementation: `specification/crs/georeferencing.md`,
  `crates/ptiff-rust/examples/write_ptiff_fixture.rs`.
