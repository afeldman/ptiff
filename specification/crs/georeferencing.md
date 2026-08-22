# PTIFF Specification — CRS Domain

**Status:** Draft
**Version:** 0.1.0
**Private tag:** 65003 (`PtiffCrs`)
**Storage notation:** `ptiff.crs.*`
**Normative basis:** `RFC-0001-Core.md`, `RFC-0002-GeoTIFF.md`, reference implementation
`scripts/gen_interop_fixture.cpp`.

---

## 1. Purpose

The CRS domain records how a planetary dataset is **georeferenced**: which planetary body it
depicts, which reference frame realization is used, and which map projection maps body
coordinates to the image plane. PTIFF deliberately reuses established georeferencing
conventions (see RFC-0002) rather than inventing a parallel mechanism, so that PTIFF-aware
tooling can interoperate with the GeoTIFF / GIS ecosystem.

> PTIFF SHOULD prefer reusing existing, well-established conventions (e.g., GeoTIFF tag
> conventions, established CRS identifiers, SPICE kernel conventions) over inventing new
> ones. — RFC-0001, §8 Design Principles

---

## 2. Field register

| Field                        | Type   | Required | Meaning                                         |
|------------------------------|--------|----------|-------------------------------------------------|
| `ptiff.crs.body`             | string | no       | Planetary body identifier, e.g. a NAIF body ID such as `301` (the Moon) |
| `ptiff.crs.projection`       | string | no       | Map projection name, e.g. `equirectangular`     |
| `ptiff.crs.reference_frame`  | string | no       | Reference frame / datum realization, e.g. `IAU_MOON_2000` |

### 2.1 Example values (from the interop fixture)

```
ptiff.crs.body            = 301
ptiff.crs.projection      = equirectangular
ptiff.crs.reference_frame = IAU_MOON_2000
```

---

## 3. Semantics

- **`body`** identifies the planetary body. NAIF body IDs (`301` = the Moon, `399` = Earth,
  `499` = Mars, …) are the canonical form. Other recognized identifiers MAY be used and
  SHOULD be registered in the field register appendix.
- **`projection`** names the map projection. The reference implementation exercises
  `equirectangular`; additional projections are future work.
- **`reference_frame`** names the frame/datum realization that ties body coordinates to
  inertial/world coordinates (e.g. a planet-fixed frame). When combined with the Camera
  extrinsics, this completes the description of how image pixels map onto the planetary
  surface.

---

## 4. Interoperability with GeoTIFF

Per RFC-0002, PTIFF reuses GeoTIFF's CRS conventions where they apply to non-Earth bodies:

- For Earth-centric data the established GeoTIFF tags (33550–34735) SHOULD be honoured and
  the `ptiff.crs.*` fields provide the planetary-science overlay.
- Where a PTIFF extension domain overlaps in purpose with an established convention (e.g.
  GeoTIFF's georeferencing tags), PTIFF SHOULD reuse or explicitly extend that convention
  rather than define a parallel, incompatible mechanism — unless a documented
  planetary-science-specific limitation requires otherwise (RFC-0001 §11).
- A GIS tool SHOULD be able to load a PTIFF orthomosaic using planetary CRS metadata
  analogous to how GeoTIFF is used for Earth basemaps today (RFC-0001 §9, UC4).

---

## 5. References

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65003 payload encoding.
- [`RFC-0002-GeoTIFF.md`](../../rfcs/RFC-0002-GeoTIFF.md) — GeoTIFF relationship / interop.
- [`../camera/calibration.md`](../camera/calibration.md) — intrinsics/extrinsics used to map
  pixels to the surface.
