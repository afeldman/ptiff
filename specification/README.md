# PTIFF Specification

This directory contains the **official PTIFF specification** — the normative description of
the Planetary TIFF file format. It is a strict extension of TIFF 6.0 / BigTIFF that carries
planetary-science metadata in private tags so the baseline image remains readable by the broad
existing TIFF ecosystem.

- **License:** the specification is provided under the Apache License 2.0 (see
  [`LICENSE-SPEC`](../LICENSE-SPEC)).
- **Conceptual basis:** [`rfcs/RFC-0001-Core.md`](../rfcs/RFC-0001-Core.md) (motivation,
  goals, design principles) and [`rfcs/RFC-0002-GeoTIFF.md`](../rfcs/RFC-0002-GeoTIFF.md)
  (GeoTIFF relationship).

## Status

All documents are **Draft**. Content may still change during review. Each document states its
own `Status` and `Version`.

## Reading order

| Step | Read | Why |
|------|------|-----|
| 1 | [`core/`](./core/) | Container, private tags, payload encoding, baseline metadata |
| 2 | [`camera/`](./camera/) | `ptiff.camera.*` calibration (tag 65002) |
| 3 | [`spice/`](./spice/) | `ptiff.spice.*` pointing (tag 65001) |
| 4 | [`crs/`](./crs/) | `ptiff.crs.*` georeferencing (tag 65003) |
| 5 | [`appendices/`](./appendices/) | Field register and tag table |
| 6 | the reserved domains | [`photometry/`](./photometry/), [`stereo/`](./stereo/), [`mesh/`](./mesh/), [`ai/`](./ai/) |

## Directory layout

```
specification/
├── README.md            This overview
├── core/                Container + private tags 65001–65005 + payload encoding
│   └── container-encoding.md
├── camera/              ptiff.camera.* (tag 65002)
│   └── calibration.md
├── spice/               ptiff.spice.* (tag 65001)
│   └── pointing.md
├── crs/                 ptiff.crs.* (tag 65003)
│   └── georeferencing.md
├── photometry/          Radiometric calibration (conceptual)
│   └── calibration.md
├── stereo/              Stereo pair relationships (conceptual)
│   └── pair-metadata.md
├── mesh/                3-D mesh / geometry (conceptual)
│   └── 3d-representation.md
├── ai/                  AI / ML-derived products (conceptual)
│   └── ai-products.md
└── appendices/          Field register + tag table
    ├── field-register.md
    └── tag-table.md
```

## Private tags at a glance

| Tag | Domain | Prefix |
|-----|--------|--------|
| 65001 | SPICE | `ptiff.spice.*` |
| 65002 | Camera | `ptiff.camera.*` |
| 65003 | CRS | `ptiff.crs.*` |
| 65004 | Scientific Layers | `ptiff.layers.*` |
| 65005 | Provenance | `ptiff.provenance.*` |

See [`appendices/tag-table.md`](./appendices/tag-table.md) for details.

## Conformance

Conformance requirements sit in each domain document and in
[`core/container-encoding.md`](./core/container-encoding.md) §6. The executable conformance
suite lives in [`../conformance/`](../conformance/).
