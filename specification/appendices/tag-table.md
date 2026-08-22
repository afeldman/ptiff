# PTIFF Specification — Tag Table

Normative reference table of the private TIFF tags reserved by PTIFF. All five live in the
TIFF private/extension range (≥ `0x8000` = 32768) and are `BYTE`-typed with a versioned PTIFF
payload; see `../core/container-encoding.md` §3 and §4.

| Tag | Name | Field type | Payload | Domain |
|-----|------|-----------|---------|--------|
| 65001 | `PtiffSpice` | BYTE (1) | versioned PTIFF | `ptiff.spice.*` |
| 65002 | `PtiffCameraGeometry` | BYTE (1) | versioned PTIFF | `ptiff.camera.*` |
| 65003 | `PtiffCrs` | BYTE (1) | versioned PTIFF | `ptiff.crs.*` |
| 65004 | `PtiffScientificLayers` | BYTE (1) | versioned PTIFF | `ptiff.layers.*` |
| 65005 | `PtiffProvenance` | BYTE (1) | versioned PTIFF | `ptiff.provenance.*` |

Notes:

- The tag range 65001–65005 was chosen to avoid collision with common registered extensions
  (GeoTIFF 33550–34735, GDAL 42112–42113).
- No additional tags are currently allocated. New extension domains (photometry, stereo, mesh,
  AI) are expected to be realized on the existing Scientific-Layers (65004) and Provenance
  (65005) tags or to request new allocations through the RFC process (RFC-0001 §17 item 2).

Reference implementation: `libptiff/include/ptiff/io/backend/tiff/tiff_tag.hpp`.
