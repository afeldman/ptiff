# PTIFF Specification — CRS Domain

The CRS domain describes how a planetary dataset is georeferenced: the planetary body,
reference frame realization and map projection. It is stored in private TIFF tag 65003
(`PtiffCrs`) as the `ptiff.crs.*` field group, and reuses GeoTIFF CRS conventions for
interoperability.

## Documents

| Document | Contents |
|----------|----------|
| [`georeferencing.md`](./georeferencing.md) | CRS field register, semantics, GeoTIFF interoperability |

## Related

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65003 payload encoding.
- [`RFC-0002-GeoTIFF.md`](../../rfcs/RFC-0002-GeoTIFF.md) — GeoTIFF relationship / interop.
- [`../camera/calibration.md`](../camera/calibration.md) — intrinsics/extrinsics used to map
  pixels to the surface.
