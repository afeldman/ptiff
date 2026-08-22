# PTIFF Specification — Mesh / 3D Domain

**Status:** Draft — conceptual; no dedicated private tag allocated yet
**Storage notation:** future `ptiff.mesh.*` (concept)
**Normative basis:** `RFC-0001-Core.md` (§7, §9 UC3, §17 item 8).

---

## 1. Purpose

The Mesh / 3D domain describes how **3-D geometric data** (meshes, point clouds, terrain
models) is encoded or referenced from a PTIFF file. RFC-0001's use case:

> **UC3 — Derived depth/mesh product distribution.** A processing pipeline emits a PTIFF file
> whose baseline image is a depth map or shaded relief, with an attached mesh or 3D-geometry
> accompanying it. — RFC-0001 §9

And the design principle that large derived products are kept out of the main raster unless
appropriate:

> Where large derived products (e.g., dense meshes, large auxiliary rasters) are attached to a
> PTIFF file, they SHOULD be kept in a manner that does not burden the raster path — via
> referenced external resources, overviews, or companion files — rather than inline. —
> RFC-0001 §9/supporting principle

---

## 2. Status and current implementation

The Mesh / 3D domain is **conceptual / reserved**: no dedicated private tag is currently
allocated and no `ptiff.mesh.*` field schema is yet defined. RFC-0001 §17 item 8 states the
required scope:

> **Mesh/3D representation** — how mesh or other 3D geometric data is encoded or referenced
> from a PTIFF file. — RFC-0001 §17 item 8

Two design constraints already shape how a future Mesh RFC must be written:

1. **3-D data is generally not pixel-aligned** and is often large. Per the design principle
   above, meshes are expected to be referenced or stored as companion resources rather than
   forced into the raster layer model.
2. **Multi-file relationships** (§17 item 13) are the natural vehicle for referencing a mesh
   file from a raster PTIFF; the Mesh domain SHOULD align with that future work.

---

## 3. Planned scope (reserved for a future extension RFC)

When the Mesh / 3D extension RFC is written it SHOULD specify:

- how a mesh / point-cloud is **referenced** or **embedded** (inline encoding vs. companion
  resource, MIME/format),
- the mesh's **coordinate frame** relationship to the CRS domain and/or to the camera
  extrinsics,
- unit and axis-convention requirements,
- how depth-map / shaded-relief baseline images relate to an attached mesh,
- alignment with multi-file linkage (§17 item 13).

---

## 4. Conformance

There are currently no normative mesh requirements beyond the generic container rules. A
PTIFF reader MUST tolerate a file with no mesh/3D metadata.

---

## 5. References

- [`../crs/georeferencing.md`](../crs/georeferencing.md) — body frame / datum for 3-D geometry.
- [`../camera/calibration.md`](../camera/calibration.md) — intrinsics/extrinsics and projection.
- [`../core/container-encoding.md`](../core/container-encoding.md) — record rules.
- `RFC-0001-Core.md` — §7, §9 UC3, §17 items 8 and 13.
