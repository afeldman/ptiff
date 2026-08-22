# PTIFF Specification — Stereo Domain

The Stereo domain represents stereo pair relationships, relative pose, and disparity /
correspondence metadata. It is currently **conceptual / reserved** (no dedicated private tag)
and builds on the Camera domain for per-view calibration and on the Scientific-Layers tag for
depth/disparity layer products.

## Documents

| Document | Contents |
|----------|----------|
| [`pair-metadata.md`](./pair-metadata.md) | Purpose, current status, planned scope |

## Related

- [`../camera/calibration.md`](../camera/calibration.md) — per-view intrinsics/extrinsics.
- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65004 and record rules.
- `RFC-0001-Core.md` — §9 UC2, §17 items 6 and 13.
