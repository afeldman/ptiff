# RFC-0003: Camera Models

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
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reflects the camera-intrinsics/extrinsics support shipped by the reference implementation (`ptiff.camera.*`, private tag 65002). |

---

## 1. Abstract

This RFC defines the **Camera domain** of PTIFF — the acquisition geometry of one sensor:
intrinsic parameters (focal length, principal point, in pixels), extrinsic pose (rotation
quaternion + world translation), and an observation timestamp. It is carried by the private
tag **65002** (`PtiffCameraGeometry`) under the storage prefix `ptiff.camera.*`. From these
scalar fields the derived matrices `K`, `[R | t]`, and `P = K [R | t]` are synthesized at read
time. This RFC resolves RFC-0001 §17 item 3.

---

## 2. Status

Draft. The field schema and matrices below are normative and match the reference
implementation (`bindings/rust/src/lib.rs` `Camera`, `bindings/c/ptiff_camera.h`).

---

## 3. Camera model

`ptiff.camera.model` names the camera model. PTIFF defines one required model today:

| Value | Meaning |
|-------|---------|
| `pinhole` | Perspective pinhole projection (§5). |

The value is a free-form string. A reader that sees an unrecognized model name MUST still
decode the scalar fields (§4); the matrix derivations in §5 are defined for the **pinhole**
model and are the normative ones. `model` defaults to `"pinhole"` when a file carries camera
intrinsics but no explicit `ptiff.camera.model`.

---

## 4. Field register

All values on disk are UTF-8 decimal strings, encoded per RFC-7002 §4. The table below is the
canonical `ptiff.camera.*` schema.

### 4.1 Intrinsics

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.camera.focal_length_x` | f64 | if intrinsics | Focal length along the horizontal image axis, px |
| `ptiff.camera.focal_length_y` | f64 | if intrinsics | Focal length along the vertical image axis, px |
| `ptiff.camera.principal_x` | f64 | if intrinsics | Principal point X (column axis), px |
| `ptiff.camera.principal_y` | f64 | if intrinsics | Principal point Y (row axis), px |

### 4.2 Extrinsics

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.camera.rotation_w` | f64 | if extrinsics | Rotation quaternion real part (w,x,y,z) |
| `ptiff.camera.rotation_x` | f64 | if extrinsics | Rotation quaternion X |
| `ptiff.camera.rotation_y` | f64 | if extrinsics | Rotation quaternion Y |
| `ptiff.camera.rotation_z` | f64 | if extrinsics | Rotation quaternion Z |
| `ptiff.camera.position_x` | f64 | if extrinsics | Camera position, world X |
| `ptiff.camera.position_y` | f64 | if extrinsics | Camera position, world Y |
| `ptiff.camera.position_z` | f64 | if extrinsics | Camera position, world Z |

### 4.3 Common

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.camera.model` | string | no | Camera model name (`pinhole`); defaults to `pinhole` |
| `ptiff.camera.timestamp` | string | no | ISO-8601 UTC observation timestamp, e.g. `2026-08-21T12:34:56.000Z`; empty/unset otherwise |

### 4.4 Presence rules

- The **intrinsics group** is present iff any of `focal_length_*` / `principal_*` is present.
- The **extrinsics group** is present iff any of `rotation_*` / `position_*` is present.
- A file MAY carry intrinsics only, extrinsics only, both, or neither.
- A zero value in `focal_length_*` denotes **unset** (not an infinite focal length).
- The booleans `has_intrinsics` / `has_extrinsics` and the matrices in §5 are **derived at
  read time** from the scalar fields above; they are NOT stored as `ptiff.camera.*` strings.

---

## 5. Matrices

All matrices are row-major doubles and are **derived** — computed by the library on read, not
stored as extension fields.

### 5.1 Intrinsic matrix K (3×3)

```
K = [ fx   0  cx ]
    [  0  fy  cy ]
    [  0   0   1 ]
```

### 5.2 Extrinsic matrix [R | t] (3×4)

`R` is the camera-to-world rotation derived from the quaternion `(w, x, y, z)`; `t` is the
camera position `(position_x, position_y, position_z)` in world coordinates.

```
[R | t]   (3×4, row-major)
```

### 5.3 Projection matrix P (3×4)

```
P = K * [R | t]    (3×4, row-major)
```

`P` maps a world point `X` to a homogeneous image point: `x ~ P * X`. It is available whenever
both the intrinsics and extrinsics groups are present; otherwise it is zero-filled.

---

## 6. Conformance

- A **writer** MUST serialize intrinsics/extrinsics into the matching `ptiff.camera.*` fields on
  tag 65002 using the RFC-7002 codec.
- A **reader** MUST decode the scalar fields even when `ptiff.camera.model` names an unknown
  model; MUST derive `K`, `[R | t]`, `P` per §5.
- A PTIFF reader MUST tolerate a file with no camera metadata.

---

## 7. References

- `RFC-0001-Core.md` — §9 UC1, §17 item 3.
- `RFC-7002` — payload codec, tag 65002.
- Reference implementation: `specification/camera/calibration.md`,
  `bindings/rust/src/lib.rs` (`Camera`), `bindings/c/ptiff_camera.h`.
