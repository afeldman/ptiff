# PTIFF Specification — Camera Domain

**Status:** Draft
**Version:** 0.1.0
**Private tag:** 65002 (`PtiffCameraGeometry`)
**Storage notation:** `ptiff.camera.*`
**Normative basis:** `RFC-0001-Core.md`, reference implementation
`crates/ptiff-core/src/geometry/camera.rs` (`Camera`); the C ABI `ptiff_camera`
is in `crates/ptiff-c` (cbindgen-authorised into `target/ptiff_c.h`).

---

## 1. Purpose

The Camera domain records the acquisition geometry of one sensor: its **intrinsic**
parameters (focal length and principal point, in pixels), its **extrinsic** pose (rotation
quaternion + world translation), and an **observation timestamp**. From these, the derived
intrinsic matrix `K`, the extrinsic matrix `[R | t]` and the full projection matrix
`P = K * [R | t]` can be synthesized.

---

## 2. Camera model

`ptiff.camera.model` names the camera model. PTIFF defines one required model today:

| Value     | Meaning                                  |
|-----------|------------------------------------------|
| `pinhole` | Perspective pinhole projection (§5).      |

The value is a free-form string. A reader that sees an unrecognized model name MUST still
decode the scalar fields; the matrix derivations in §5 are defined as the **pinhole** model
and are the normative ones. `model` defaults to `"pinhole"` when a file carries camera
intrinsics but no explicit `ptiff.camera.model`.

---

## 3. Field register

All values on disk are UTF-8 decimal strings (see `core/container-encoding.md` §4). The table
below is the canonical `ptiff.camera.*` schema.

### 3.1 Intrinsics

| Field                     | Type   | Required | Meaning                                   |
|---------------------------|--------|----------|-------------------------------------------|
| `ptiff.camera.focal_length_x` | f64 | if intrinsics | Focal length along the horizontal image axis, px |
| `ptiff.camera.focal_length_y` | f64 | if intrinsics | Focal length along the vertical image axis, px   |
| `ptiff.camera.principal_x`    | f64 | if intrinsics | Principal point X (column axis), px       |
| `ptiff.camera.principal_y`    | f64 | if intrinsics | Principal point Y (row axis), px          |

### 3.2 Extrinsics

| Field                    | Type   | Required | Meaning                              |
|--------------------------|--------|----------|--------------------------------------|
| `ptiff.camera.rotation_w`    | f64 | if extrinsics | Rotation quaternion real part (w,x,y,z) |
| `ptiff.camera.rotation_x`    | f64 | if extrinsics | Rotation quaternion X                |
| `ptiff.camera.rotation_y`    | f64 | if extrinsics | Rotation quaternion Y                |
| `ptiff.camera.rotation_z`    | f64 | if extrinsics | Rotation quaternion Z                |
| `ptiff.camera.position_x`    | f64 | if extrinsics | Camera position, world X             |
| `ptiff.camera.position_y`    | f64 | if extrinsics | Camera position, world Y             |
| `ptiff.camera.position_z`    | f64 | if extrinsics | Camera position, world Z             |

### 3.3 Common

| Field                  | Type   | Required | Meaning                              |
|------------------------|--------|----------|--------------------------------------|
| `ptiff.camera.model`   | string | no       | Camera model name (`pinhole`); defaults to `pinhole` |
| `ptiff.camera.timestamp` | string | no     | ISO-8601 UTC observation timestamp, e.g. `2026-08-21T12:34:56.000Z`; empty/unset otherwise |

### 3.4 Presence rules

- **Intrinsics group** is present iff any of `focal_length_*` / `principal_*` is present.
- **Extrinsics group** is present iff any of `rotation_*` / `position_*` is present.
- A file may carry intrinsics only, extrinsics only, both, or neither.
- A zero value in `focal_length_*` denotes **unset** (not an infinite focal length).
- The booleans `has_intrinsics` / `has_extrinsics` and the matrices in §4 are **derived at
  read time** from the scalar fields above. They are NOT stored as `ptiff.camera.*` strings;
  they are computed values surfaced in the runtime model / ABI structures / JSON output.

---

## 4. Matrices

All matrices are **row-major doubles**. They are **derived** — computed by the library on read
— and are NOT stored as extension fields.

### 4.1 Intrinsic matrix K (3×3)

```
K = [ fx   0  cx ]
    [  0  fy  cy ]
    [  0   0   1 ]
```

### 4.2 Extrinsic matrix [R | t] (3×4)

`R` is the camera-to-world rotation matrix derived from the rotation quaternion
`(w, x, y, z)`; `t` is the camera position `(position_x, position_y, position_z)` in world
coordinates:

```
[R | t]  (3×4, row-major)
```

### 4.3 Projection matrix P (3×4)

```
P = K * [R | t]       (3×4, row-major)
```

`P` maps a world point `X` to a homogeneous image point via `x ~ P * X`. It is available
whenever both the intrinsics and the extrinsics groups are present; otherwise it is
zero-filled.
