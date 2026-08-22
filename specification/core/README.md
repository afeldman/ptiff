# PTIFF Specification — Core Domain

The Core domain defines the foundation that every other PTIFF domain builds on: the
TIFF/BigTIFF container, the five private extension tags, the versioned extension payload
format, and the core image metadata model.

## Documents

| Document | Contents |
|----------|----------|
| [`container-encoding.md`](./container-encoding.md) | Container model, private tags 65001–65005, byte-payload format, core image metadata fields, conformance |

## Normative sources

- `rfcs/RFC-0001-Core.md` — motivation, goals, design principles, terminology.
- `rfcs/RFC-0002-GeoTIFF.md` — relationship to / interop with GeoTIFF.
- `libptiff/src/io/backend/tiff/` — reference implementation of the encoding.

## Extension domains

Each of the following directories defines one scientific domain and records the
`ptiff.<domain>.*` field names it owns:

- [`../camera/`](../camera/) — `ptiff.camera.*` (tag 65002)
- [`../spice/`](../spice/) — `ptiff.spice.*` (tag 65001)
- [`../crs/`](../crs/) — `ptiff.crs.*` (tag 65003)
- [`../photometry/`](../photometry/) — photometric / radiometric calibration
- [`../stereo/`](../stereo/) — stereo geometry and derived products
- [`../mesh/`](../mesh/) — 3-D mesh products
- [`../ai/`](../ai/) — AI / ML-derived products
- [`../appendices/`](../appendices/) — field register, tag tables, normative references
