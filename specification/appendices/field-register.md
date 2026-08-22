# PTIFF Specification — Field Register

This appendix is the **normative register** of every well-known `ptiff.*` field name. It is
the single source of truth for field names across all domains. A new field MUST be added here
before it is used; unknown fields are preserved verbatim by readers (see
`core/container-encoding.md` §4.3).

---

## 1. Tag → domain mapping

| Tag | Domain | Prefix |
|-----|--------|--------|
| 65001 | SPICE | `ptiff.spice.` |
| 65002 | Camera | `ptiff.camera.` |
| 65003 | CRS | `ptiff.crs.` |
| 65004 | Scientific Layers | `ptiff.layers.` |
| 65005 | Provenance | `ptiff.provenance.` |

---

## 2. Registered fields

### 2.1 SPICE (`ptiff.spice.*`, tag 65001)

| Field | Example |
|-------|---------|
| `ptiff.spice.frame` | `IAU_MOON` |
| `ptiff.spice.instrument` | `LROC_NAC` |
| `ptiff.spice.time_system` | `TDB` |
| `ptiff.spice.observation_time` | `2026-08-20T00:00:00.000` |

### 2.2 Camera (`ptiff.camera.*`, tag 65002)

| Field | Example |
|-------|---------|
| `ptiff.camera.model` | `pinhole` |
| `ptiff.camera.focal_length_x` | `700.0` |
| `ptiff.camera.focal_length_y` | `700.0` |
| `ptiff.camera.principal_x` | `64.0` |
| `ptiff.camera.principal_y` | `64.0` |
| `ptiff.camera.rotation_w` | `1.0` |
| `ptiff.camera.rotation_x` | `0.0` |
| `ptiff.camera.rotation_y` | `0.0` |
| `ptiff.camera.rotation_z` | `0.0` |
| `ptiff.camera.position_x` | `0.0` |
| `ptiff.camera.position_y` | `0.0` |
| `ptiff.camera.position_z` | `100.0` |
| `ptiff.camera.timestamp` | `2026-08-21T12:34:56.000Z` |

### 2.3 CRS (`ptiff.crs.*`, tag 65003)

| Field | Example |
|-------|---------|
| `ptiff.crs.body` | `301` |
| `ptiff.crs.projection` | `equirectangular` |
| `ptiff.crs.reference_frame` | `IAU_MOON_2000` |

### 2.4 Scientific Layers (`ptiff.layers.*`, tag 65004)

| Field | Example |
|-------|---------|
| `ptiff.layers.dem` | `dem_16x16` |
| `ptiff.layers.albedo` | `albedo_16x16` |
| `ptiff.layers.confidence` | `confidence_16x16` |

### 2.5 Provenance (`ptiff.provenance.*`, tag 65005)

| Field | Example |
|-------|---------|
| `ptiff.provenance.software` | `libptiff-0.3.0` |
| `ptiff.provenance.operator` | `interop-fixture-generator` |
| `ptiff.provenance.commit` | `f73dec27469f878a8de9d209e0c81bf33e8b8d0a` |

---

## 3. Reserved / future domains

The following prefixes are reserved by the domain structure but have **no normative field
schema yet**. Fields MAY use them on a best-effort basis (they round-trip per §4.3 of the
core spec) but are not yet registered:

| Prefix | Status |
|--------|--------|
| `ptiff.photometry.*` | Conceptual (see `../photometry/calibration.md`) |
| `ptiff.stereo.*` | Conceptual (see `../stereo/pair-metadata.md`) |
| `ptiff.mesh.*` | Conceptual (see `../mesh/3d-representation.md`) |
| `ptiff.ai.*` | Conceptual (see `../ai/ai-products.md`) |

---

## 4. Adding a field

1. Define the field in the owning domain document.
2. Add it to the table above with a concrete example value.
3. Register any tag-type or value-domain constraints in the domain document.
4. Add a round-trip coverage assertion in the conformance suite (`conformance/`).
