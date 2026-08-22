# PTIFF Specification — Mesh / 3D Domain

The Mesh / 3D domain describes how 3-D geometric data (meshes, point clouds, terrain models)
is encoded or referenced from a PTIFF file. It is currently **conceptual / reserved** (no
dedicated private tag) and is expected to link to future multi-file relationship work.

## Documents

| Document | Contents |
|----------|----------|
| [`3d-representation.md`](./3d-representation.md) | Purpose, current status, planned scope |

## Related

- [`../crs/georeferencing.md`](../crs/georeferencing.md) — body frame / datum for 3-D geometry.
- [`../camera/calibration.md`](../camera/calibration.md) — intrinsics/extrinsics and projection.
- [`../core/container-encoding.md`](../core/container-encoding.md) — record rules.
- `RFC-0001-Core.md` — §7, §9 UC3, §17 items 8 and 13.
