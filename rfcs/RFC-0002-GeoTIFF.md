# RFC-0002: GeoTIFF Relationship & Interoperability

**Status:** Draft
**Category:** Reference
**Requires:** RFC-0001 (PTIFF Core)
**Obsoletes:** None

---

## 1. Abstract

This document compares PTIFF with GeoTIFF: what each format is for, where they differ, and
how PTIFF deliberately stays interoperable with GeoTIFF conventions. It is a reference /
context document — it clarifies the conceptual relationship to GeoTIFF and lays groundwork
for the later CRS / planetary georeferencing extension domain (RFC-0001 §17, item 4), rather
than defining normative binary layout or tag encodings on its own.

## 2. Status

Draft. This document records design intent and does not itself introduce normative
requirements; normative statements live in RFC-0001 and in future extension-domain RFCs.

## 3. What each format is

Both **PTIFF** and **GeoTIFF** are TIFF (or BigTIFF) based containers that embed
domain-specific metadata as tags in the TIFF file, remaining readable by the broad existing
TIFF tooling ecosystem. They differ in purpose and in the metadata they carry.

> GeoTIFF defines a small, well-known set of TIFF tags to encode 2D (and limited 2.5D) Earth
> georeferencing: map projections, coordinate reference systems, and pixel-to-CRS transforms.
> GeoTIFF is deliberately narrow in scope: it addresses Earth-centric, map-projection-based
> georeferencing and nothing else. It has no concept of camera models, planetary bodies other
> than Earth in general practice, stereo pairs, SPICE-derived geometry, or 3D/AI products.
> — RFC-0001-Core §4.2

PTIFF is, in the terms of its own RFC, *"for planetary science, what GeoTIFF became for
Earth-based GIS"* (RFC-0001 §15). It carries the scientific context required to *use* a
planetary image correctly — camera model, acquisition time, SPICE-derived pointing and
position, planetary body and CRS, calibration and photometric parameters, and increasingly
depth maps, meshes, and stereo / AI-derived products — inside the same TIFF/BigTIFF container
as the pixels.

## 4. At a glance

| Aspect | GeoTIFF | PTIFF |
|---|---|---|
| **Primary use case** | 2D Earth GIS georeferencing | Planetary science image + scientific context |
| **Georeferencing** | Earth-centric, map projections, CRS, pixel→CRS transforms (2D, limited 2.5D) | Any planetary body; body IDs, reference ellipsoids, projections; reuses GeoTIFF CRS conventions |
| **Camera model** | none | yes |
| **SPICE geometry / pointing** | none | yes |
| **Acquisition time** | none | yes |
| **Photometry / calibration** | none | yes |
| **Stereo / 3D / AI products** | none | depth maps, meshes, stereo pairs, AI-derived products |
| **Processing provenance** | none | yes |
| **Band/channel model** | singular/simple images | analysis-oriented, multi-band aware |

## 5. Why PTIFF does not replace GeoTIFF

PTIFF is designed to fill a gap, not to displace adjacent standards:

> None of the above formats simultaneously provide: (a) a single self-contained file,
> (b) native readability by the broad existing TIFF/BigTIFF tooling ecosystem, and
> (c) a structured, extensible schema for planetary camera, geometric, stereo, photometric,
> and derived-product metadata. PTIFF is designed to fill exactly this gap, not to replace
> PDS4 archival practices, ISIS internal processing, or GeoTIFF's Earth-GIS niche.
> — RFC-0001-Core §4.5

## 6. Interoperability with GeoTIFF

PTIFF explicitly prefers to **reuse** established GeoTIFF conventions rather than invent
parallel mechanisms:

> PTIFF SHOULD prefer reusing existing, well-established conventions (e.g., GeoTIFF tag
> conventions, established CRS identifiers, SPICE kernel conventions) over inventing new
> ones, so that PTIFF-aware tooling can interoperate with adjacent ecosystems.
> — RFC-0001-Core §8, Design Principles: Interoperability

> Where a PTIFF extension domain overlaps in purpose with an existing, established convention
> (for example, GeoTIFF's georeferencing tags), later RFCs SHOULD reuse or explicitly extend
> that convention rather than defining a parallel, incompatible mechanism, unless a documented
> planetary-science-specific limitation requires otherwise.
> — RFC-0001-Core §11

> A GIS tool loads a PTIFF orthomosaic using planetary CRS metadata analogous to how GeoTIFF
> is used for Earth basemaps today.
> — RFC-0001-Core §9, UC4 — Georeferenced planetary basemaps

## 7. Where the formats' scopes meet: future CRS work

The concrete CRS encoding for non-Earth bodies is planned in a dedicated future
extension-domain RFC, explicitly tied to GeoTIFF:

> **Coordinate reference systems (CRS)** — how planetary body identifiers, non-Earth reference
> ellipsoids/datums, and map projections are represented, and their relationship to existing
> GeoTIFF CRS conventions.
> — RFC-0001-Core §17, item 4

This RFC serves as the contextual basis for that CRS extension domain. A future CRS RFC
SHOULD build on, and explicitly reference, the interoperability principles captured here.

## 8. Short version

- **GeoTIFF** answers "where on Earth" — narrow, mature, 2D Earth georeferencing.
- **PTIFF** answers "what and how, on which planetary body" — full scientific context
  (camera, SPICE, time, photometry, stereo/3D, provenance) in a TIFF container, additionally
  interoperable with GeoTIFF CRS conventions.
