# RFC-0008: Mesh / 3D Representation

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-0003 (Camera), RFC-0004 (CRS), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Mesh / 3D domain scope per RFC-0001 §17 items 8 and 13. |

---

## 1. Abstract

This RFC reserves the **Mesh / 3D domain** of PTIFF: how a mesh or other 3-D geometric data is
encoded in, or referenced from, a PTIFF file. It sits alongside the vector data that describes
how depth maps / shaded-relief baseline images relate to an attached 3-D surface. This RFC is
currently **conceptual / reserved**; it records the required scope, resolving RFC-0001 §17 item
8 and aligning with item 13 (multi-file relationships).

---

## 2. Status and existing building blocks

The Mesh / 3D domain is reserved. The pieces a future RFC can build on already exist:

- **CRS domain (RFC-0004)** — planetary body frame / datum for 3-D geometry.
- **Camera domain (RFC-0003)** — intrinsics / extrinsics and projection for associating image
  pixels with surface points.
- **Scientific-Layers tag (65004)** — can carry 2.5-D rasters such as depth maps referenced by
  a mesh.

A mesh is a *multidimensional companion* to a raster PTIFF; the Mesh domain SHOULD align with
that future work and with multi-file linkage.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Mesh / 3D RFC SHOULD specify:

- how a mesh / point-cloud is **referenced** or **embedded** (inline encoding vs. companion
  resource, MIME / format),
- the mesh's **coordinate frame** relationship to the CRS domain and / or to the camera
  extrinsics,
- unit and axis-convention requirements,
- how depth-map / shaded-relief baseline images relate to an attached mesh,
- alignment with multi-file linkage (RFC-0001 §17 item 13).

---

## 4. Conformance

There are currently no normative mesh requirements beyond the generic container rules of
RFC-7002. A PTIFF reader MUST tolerate a file with no mesh / 3-D metadata.

---

## 5. References

- `RFC-0001-Core.md` — §7, §9 UC3, §17 items 8 and 13.
- `RFC-0003` — Camera domain (intrinsics / extrinsics / projection).
- `RFC-0004` — CRS domain (body frame / datum for 3-D geometry).
- `RFC-7002` — payload codec.
- Reference implementation: `specification/mesh/3d-representation.md`.
