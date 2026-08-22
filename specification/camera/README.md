# PTIFF Specification — Camera Domain

The Camera domain describes the acquisition geometry of a sensor: pinhole intrinsics,
extrinsic pose, observation timestamp, and the derived matrices `K`, `[R | t]` and
`P = K * [R | t]`. It is stored in private TIFF tag 65002 (`PtiffCameraGeometry`) as the
`ptiff.camera.*` field group.

## Documents

| Document | Contents |
|----------|----------|
| [`calibration.md`](./calibration.md) | Camera model, full `ptiff.camera.*` field register, matrix derivations |

## Related

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65002 payload encoding.
- [`../spice/`](../spice/) — `ptiff.spice.*` pointing/position kernels (sensor pose source).
