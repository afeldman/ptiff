# PTIFF Specification — SPICE Domain

The SPICE domain records NAIF-SPICE-derived acquisition context: reference frame, instrument,
time system and observation time. It is stored in private TIFF tag 65001 (`PtiffSpice`) as the
`ptiff.spice.*` field group.

## Documents

| Document | Contents |
|----------|----------|
| [`pointing.md`](./pointing.md) | SPICE field register, semantics, relationship to the camera extrinsics |

## Related

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65001 payload encoding.
- [`../camera/calibration.md`](../camera/calibration.md) — explicit camera extrinsics.
