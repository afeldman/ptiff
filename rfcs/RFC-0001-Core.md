# RFC-0001: PTIFF Core

**Status:** Approved
**Category:** Foundational
**Requires:** None
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-03

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-03 | PTIFF Maintainers | Prepared for approval: added document metadata, aligned the Status section with the governance process (`GOVERNANCE.md` §4), editorial consistency pass. |
| 0.1.0   | 2026-09-22 | Anton Feldmann | Approved after the formal review opened 2026-09-08 and closed 2026-09-22; no formal objections were raised. |

---

## 1. Abstract

This document defines the motivation, problem statement, goals, non-goals, and design principles
of PTIFF (Planetary TIFF), an open, scientific image data standard for planetary imagery, stereo
vision, georeferencing, camera modeling, and derived 3D and AI data products.

PTIFF is defined as a strict extension of TIFF and BigTIFF. It does not introduce a new binary
container format. Instead, it defines a structured, namespaced way of attaching planetary-science
metadata to TIFF/BigTIFF files so that:

- generic, unmodified TIFF readers MUST be able to read the primary (baseline) image, and
- PTIFF-aware software MAY additionally read structured planetary, camera, geometric, stereo,
  photometric, and derived-product metadata from the same file.

This document establishes the conceptual and normative foundation for all subsequent PTIFF RFCs.
It does not define binary layout, tag numbers, tag encodings, or any other implementation detail.
Those are deferred to later, more specific RFCs, enumerated in [Section 17](#17-open-questions).

## 2. Status

This RFC was approved under the PTIFF governance process
([`GOVERNANCE.md`](../GOVERNANCE.md), §4) on 2026-09-22 by Anton Feldmann, following the
formal review opened on 2026-09-08. No formal objections were raised. It has not been ratified
by any standardization body.

Once this RFC is **Approved**, subsequent changes MUST be additive and backward compatible
(see Section 8, "Backward Compatibility"), or, where that is not appropriate, introduced only
through a new RFC that supersedes this one. An **Approved** RFC that has reached broad
acceptance MAY subsequently be declared **Stable**; the criteria and process are defined in
[`GOVERNANCE.md`](../GOVERNANCE.md), §4.

This RFC MUST be read as the entry point to the PTIFF specification. Where any other PTIFF
document conflicts with this RFC, this RFC takes precedence unless it is explicitly superseded.

## 3. Motivation

Planetary science, robotics, and space missions increasingly produce large volumes of imagery
that is not merely a 2D picture but a **measurement**: it carries a camera model, a time of
acquisition, a pointing and position (often derived from SPICE kernels), a planetary body and
coordinate reference system, calibration and photometric parameters, and increasingly,
AI-derived or photogrammetrically-derived products such as depth maps, meshes, and stereo
correspondences.

Today, this information is scattered across:

- proprietary or mission-specific ancillary files,
- loosely-documented PDS3/PDS4 label structures,
- ISIS cube (`.cub`) sidecar metadata understood only by ISIS,
- GeoTIFF tags, which encode only 2D Earth-centric georeferencing,
- ad hoc `.json`, `.txt`, or `.xml` sidecar files with no common schema.

This fragmentation means that:

- no common tool can open a planetary image and get more than pixels,
- reproducibility of scientific results depends on undocumented, mission-specific tooling,
- interoperability between agencies, universities, and open-source tools is low,
- combining datasets from different missions or instruments requires bespoke conversion code.

PTIFF exists to give the planetary science community a single, open, TIFF-compatible container
that can carry both the image and the scientific context required to use it correctly, without
sacrificing compatibility with the enormous existing ecosystem of TIFF tooling.

## 4. Background

### 4.1 TIFF and BigTIFF

TIFF (Tag Image File Format) is a widely deployed, tag-based, extensible raster image format.
BigTIFF is a backward-compatible extension of TIFF that supports files larger than 4 GiB. Both
formats are stable, well-documented, and supported by a vast ecosystem of libraries, viewers,
and GIS tools.

### 4.2 GeoTIFF

GeoTIFF defines a small, well-known set of TIFF tags to encode 2D (and limited 2.5D) Earth
georeferencing: map projections, coordinate reference systems, and pixel-to-CRS transforms.
GeoTIFF is deliberately narrow in scope: it addresses Earth-centric, map-projection-based
georeferencing and nothing else. It has no concept of camera models, planetary bodies other
than Earth in general practice, stereo pairs, SPICE-derived geometry, or 3D/AI products.

### 4.3 PDS3 / PDS4

The Planetary Data System (PDS) label formats (PDS3, PDS4) are the standard archival format
used by NASA and partner agencies for planetary mission data. PDS4 in particular is a mature,
XML-based labeling standard with a rigorous, dictionary-driven schema and a well-established
archival and validation process.

PDS3/PDS4 labels are typically **sidecar** metadata: a separate label file describes a separate
data file. This separation is a deliberate and valid design choice for long-term archival, but
it means the image and its metadata can become desynchronized, is not self-contained as a
single file, and is not directly openable by generic image tooling. PDS3/PDS4 also does not
define a raster container format itself; the pixel data format is referenced, not owned, by the
label.

### 4.4 ISIS cubes

The USGS Integrated Software for Imagers and Spectrometers (ISIS) uses its own cube (`.cub`)
format, which embeds rich planetary metadata (camera models, SPICE-derived pointing, band
information) directly alongside pixel data. ISIS cubes are powerful but are understood natively
only by ISIS and a small number of interoperating tools; they are not TIFF-compatible and are
not intended as a general-purpose interchange format outside the ISIS ecosystem.

### 4.5 The gap PTIFF fills

None of the above formats simultaneously provide: (a) a single self-contained file, (b) native
readability by the broad existing TIFF/BigTIFF tooling ecosystem, and (c) a structured,
extensible schema for planetary camera, geometric, stereo, photometric, and derived-product
metadata. PTIFF is designed to fill exactly this gap, not to replace PDS4 archival practices,
ISIS internal processing, or GeoTIFF's Earth-GIS niche.

## 5. Problem Statement

PTIFF addresses the following concrete problems:

1. **Fragmented metadata.** Scientific context about a planetary image is split across formats
   and tools that do not interoperate.
2. **Weak reproducibility.** Without a standard, machine-readable way to carry camera models,
   SPICE-derived geometry, and processing provenance with the pixel data, reproducing a
   scientific result requires reconstructing context by hand or via mission-specific scripts.
3. **Poor tool interoperability.** GIS tools, computer vision tools, and planetary science
   tools each expect different metadata conventions, forcing lossy or manual conversion.
4. **No standard container for derived products.** Stereo-derived depth, photogrammetric
   meshes, and AI-derived products (e.g., segmentation, uncertainty maps) currently have no
   common, image-compatible home.
5. **High barrier to entry.** Because no open, TIFF-based planetary standard exists, every new
   tool or team re-solves the same metadata modeling problem independently.

PTIFF does not claim to be the first attempt to solve pieces of this problem (see Section 4);
it claims to be an attempt to solve it in a way that is simultaneously open, extensible, and
compatible with the pre-existing TIFF tooling ecosystem.

## 6. Goals

PTIFF pursues the following goals:

- **G1 — TIFF/BigTIFF compatibility.** Any PTIFF file MUST be a valid TIFF or BigTIFF file. A
  conforming generic TIFF reader MUST be able to open a PTIFF file and read at least the
  baseline image.
- **G2 — Self-contained scientific context.** A PTIFF file SHOULD be able to carry camera
  model, geometric, coordinate reference system, photometric, and provenance metadata within
  the same file as the pixel data it describes.
- **G3 — Extensibility without fragmentation.** New scientific domains (e.g., new instrument
  types, new derived-product types) MUST be addable via new, independently versioned RFCs
  without breaking existing readers or requiring a new container format.
- **G4 — Open specification and governance.** The PTIFF specification MUST be developed and
  published openly, under an open license, with a transparent governance process (see
  [`GOVERNANCE.md`](../GOVERNANCE.md)).
- **G5 — Interoperability across domains.** PTIFF SHOULD serve as a common interchange point
  between GIS tooling, computer vision / photogrammetry pipelines, and planetary science
  archival systems, without requiring any of them to abandon their existing formats.
- **G6 — Streaming and cloud readiness.** PTIFF SHOULD support access patterns compatible with
  partial reads, tiling, and cloud/object-storage-friendly access, consistent with modern
  cloud-optimized raster practice.
- **G7 — Long-term archival suitability.** PTIFF SHOULD be suitable, in conjunction with
  archival practices such as PDS4, as a durable interchange and working format, not only a
  transient processing format.

## 7. Non-Goals

PTIFF explicitly does **not** aim to:

- **NG1 — Replace PDS3/PDS4.** PTIFF is not an archival label format and does not aim to
  replace the PDS as the archival system of record for NASA/partner-agency missions. PTIFF MAY
  be used as an input to, or export from, PDS4 archival products, but conformance with PDS4
  archival requirements is out of scope for this RFC.
- **NG2 — Replace ISIS or mission-specific processing pipelines.** PTIFF does not aim to
  replace internal ISIS cube processing or any mission's internal working formats.
- **NG3 — Define a new low-level binary container.** PTIFF MUST NOT introduce a binary
  container format independent of TIFF/BigTIFF.
- **NG4 — Define binary layout or tag encodings in this document.** This RFC is intentionally
  free of binary-level normative detail; that is deferred to subsequent RFCs.
- **NG5 — Mandate a single implementation, library, or programming language.** PTIFF is a
  specification. Reference implementations (e.g., `libptiff`) are informative, not normative.
- **NG6 — Guarantee lossless round-tripping through arbitrary third-party TIFF-editing tools.**
  Generic TIFF tools that are unaware of PTIFF-specific tags MAY discard or fail to preserve
  them when re-saving a file. PTIFF aims for baseline-image compatibility, not universal
  metadata round-trip compatibility with all TIFF tooling.
- **NG7 — Define application-level processing algorithms.** PTIFF specifies how information is
  represented and carried, not how it is computed (e.g., it does not standardize a specific
  stereo-matching or photometric-correction algorithm).

## 8. Design Principles

- **TIFF First.** Every design decision starts from the question "does this preserve
  TIFF/BigTIFF compatibility?" This is the non-negotiable foundation of the standard, because it
  is what gives PTIFF immediate, practical value on day one: any TIFF reader can already extract
  the baseline image.
- **Backward Compatibility.** Extensions defined in later RFCs MUST be additive. A reader
  conforming to an earlier RFC version MUST NOT be broken by the presence of tags or structures
  defined in a later RFC; it MAY simply ignore what it does not understand.
- **Scientific Reproducibility.** Where PTIFF carries metadata that affects scientific
  interpretation (camera models, geometry, calibration, provenance), that metadata SHOULD be
  precise, unambiguous, and versioned, so that results derived from a PTIFF file can be
  reproduced by an independent party.
- **Interoperability.** PTIFF SHOULD prefer reusing existing, well-established conventions
  (e.g., GeoTIFF tag conventions, established CRS identifiers, SPICE kernel conventions) over
  inventing new ones, so that PTIFF-aware tooling can interoperate with adjacent ecosystems.
- **Open Specification.** The specification text, not any single implementation, is the source
  of truth. The spec MUST be publicly available and freely implementable, consistent with the
  license terms in [`LICENSE-SPEC`](../LICENSE-SPEC).
- **Extensibility.** The tag/metadata namespace MUST be structured so that new scientific
  domains can be added as independently versioned extensions without requiring changes to
  unrelated parts of the specification or to the core RFC.
- **Performance.** The format SHOULD NOT impose metadata structures that force a reader to
  parse the entire file to access the baseline image or to locate a specific metadata domain.
- **Streaming.** Where practical, the specification SHOULD favor structures that allow
  incremental or partial reads (e.g., tiled/striped access, seekable offsets) over structures
  that require the entire file to be loaded into memory.
- **Cloud Readiness.** The specification SHOULD remain compatible with cloud-optimized access
  patterns (HTTP range requests, object storage), in the spirit of established cloud-optimized
  raster practice.
- **Platform Independence.** The specification MUST NOT assume a specific operating system,
  programming language, byte order preference beyond what TIFF/BigTIFF already require, or
  proprietary toolchain.
- **Community Governance.** Changes to the specification MUST go through the open RFC and
  governance process defined in [`GOVERNANCE.md`](../GOVERNANCE.md), not through unilateral
  changes by any single implementer.

## 9. Use Cases

This section is illustrative, not normative. It motivates the goals above.

- **UC1 — Orbital/surface imagery with camera model.** A mission delivers an image with an
  embedded camera model (intrinsics, distortion) and pointing/position derived from SPICE, so
  downstream tools can project pixels to a planetary surface without external ancillary files.
- **UC2 — Stereo pair processing.** Two or more PTIFF images from a stereo rig carry enough
  geometric metadata (camera models, relative pose) for a photogrammetry pipeline to compute
  disparity or depth without additional configuration.
- **UC3 — Derived depth/mesh product distribution.** A processing pipeline emits a PTIFF file
  whose baseline image is a depth map or shaded relief, with an attached mesh or 3D-geometry
  layer, remaining readable as a plain image by generic tools.
- **UC4 — Georeferenced planetary basemaps.** A GIS tool loads a PTIFF orthomosaic using
  planetary CRS metadata analogous to how GeoTIFF is used for Earth basemaps today.
- **UC5 — AI-derived product overlay.** A machine learning pipeline attaches classification,
  segmentation, or uncertainty metadata to an image so that downstream consumers can interpret
  model output alongside the source imagery.
- **UC6 — Cross-institution interoperability.** A university research group opens imagery
  produced by a space agency's pipeline, and a commercial GIS tool opens the same file, each
  extracting the subset of metadata relevant to its purpose.

## 10. Terminology

- **PTIFF file** — A TIFF or BigTIFF file that additionally conforms to one or more PTIFF RFCs.
- **Baseline image** — The primary raster image in a PTIFF file, readable by any conforming
  generic TIFF/BigTIFF reader without knowledge of PTIFF.
- **PTIFF-aware reader** — Software that understands one or more PTIFF extension domains and
  can read the corresponding structured metadata in addition to the baseline image.
- **Extension domain** — A named, independently versioned area of the PTIFF specification
  (e.g., camera, CRS, SPICE, stereo, photometry, mesh, AI), each defined by its own RFC(s).
- **Derived product** — Data computed from one or more source images (e.g., depth maps,
  meshes, disparity maps, classification maps) that is itself carried as, or alongside, a
  PTIFF image.
- **Conformance level** — A defined subset of PTIFF requirements that an implementation claims
  to satisfy (e.g., "baseline TIFF compatibility" vs. "PTIFF Core" vs. specific extension
  domains). Precise conformance levels are defined in the conformance test suite and related
  RFCs, not in this document.
- **Normative** — Text that defines requirements an implementation MUST, SHOULD, or MAY follow
  to conform. Contrast with **informative**, which is explanatory or illustrative only.

The key words "MUST", "MUST NOT", "SHOULD", "SHOULD NOT", and "MAY" in this document are to be
interpreted as described in RFC 2119 / RFC 8174, when, and only when, they appear in all
capitals, as shown here.

## 11. Compatibility

- A PTIFF file MUST be parseable as a valid TIFF file, or, where BigTIFF is used, a valid
  BigTIFF file, per the respective base specifications.
- A generic TIFF/BigTIFF reader with no knowledge of PTIFF MUST be able to locate and decode
  the baseline image using only standard TIFF/BigTIFF mechanisms.
- PTIFF-specific metadata MUST be encoded using mechanisms already permitted by TIFF/BigTIFF
  for private or extension data (e.g., private tag ranges, IFD structures), such that a generic
  reader ignoring unknown tags remains fully functional. The specific tag numbers and encodings
  are defined in later RFCs, not in this document.
- PTIFF MUST NOT redefine the meaning of any existing baseline TIFF tag in a way that would
  cause a generic reader to misinterpret the baseline image.
- Where a PTIFF extension domain overlaps in purpose with an existing, established convention
  (for example, GeoTIFF's georeferencing tags), later RFCs SHOULD reuse or explicitly extend
  that convention rather than defining a parallel, incompatible mechanism, unless a documented
  planetary-science-specific limitation requires otherwise.

## 12. Extensibility

- The PTIFF specification is organized into a **Core** (this RFC and its direct successors) and
  a set of **extension domains**, each covering one scientific area (camera, CRS, SPICE,
  stereo, photometry, mesh/3D, AI, and others as needed).
- Each extension domain MUST be defined by its own RFC(s) and MUST be independently versioned,
  so that a domain can evolve (e.g., gain new fields) without forcing a version change in
  unrelated domains.
- A PTIFF file MAY implement any subset of extension domains. Implementing zero extension
  domains and only the Core/baseline-image requirements MUST still yield a valid PTIFF file.
- New extension domains MUST be introduced via the open RFC process and MUST NOT redefine or
  conflict with tag ranges, semantics, or identifiers already allocated to existing domains.
- Where an extension domain needs to reference external identifier systems (e.g., planetary
  body identifiers, CRS codes, SPICE kernel identifiers), it SHOULD reference existing,
  externally maintained registries rather than defining redundant identifiers, unless no
  suitable registry exists.

## 13. Security

- PTIFF inherits any parser-level security considerations applicable to TIFF/BigTIFF in
  general (e.g., malformed IFD chains, offset/length fields pointing outside the file,
  integer overflow in tag count/size fields, excessive memory allocation from crafted tag
  values). Implementations MUST treat all fields read from a PTIFF file, including PTIFF
  extension metadata, as untrusted input and MUST validate offsets, lengths, and counts before
  use.
- Because PTIFF allows arbitrary extension domains and future growth of the tag namespace,
  implementations MUST NOT assume a fixed upper bound on the number or size of extension
  structures without validating it against the actual file size and MUST guard against
  resource-exhaustion from maliciously or accidentally crafted metadata (e.g., extremely large
  claimed array counts, cyclic or self-referential IFD offsets).
- Embedded scientific metadata (e.g., camera models, coordinate transforms, provenance) is
  descriptive data, not executable content. Conforming readers MUST NOT execute, evaluate, or
  interpret any PTIFF metadata field as code.
- Detailed threat modeling and parser-hardening requirements for specific binary structures are
  deferred to the RFC(s) that define those structures (see [Section 17](#17-open-questions));
  this section establishes the general expectation, not the complete requirement set.

## 14. Performance

- Locating and decoding the baseline image SHOULD NOT require a PTIFF-aware reader to parse
  every extension domain present in the file.
- Extension domains SHOULD be structured so that a reader interested in only one domain (e.g.,
  only the camera model) is not required to parse unrelated domains (e.g., AI-derived product
  metadata) to reach it.
- Where large derived products (e.g., dense meshes, large auxiliary rasters) are attached to a
  PTIFF file, later RFCs SHOULD define mechanisms allowing readers to determine size and
  location of such data without decoding it, so that readers can make an informed decision
  about whether to load it.
- Performance requirements in this section are expressed as design pressure on later,
  binary-level RFCs; this RFC does not itself define measurable performance targets or
  benchmarks.

## 15. Long-Term Vision

PTIFF aims to become, for planetary science, what GeoTIFF became for Earth-based GIS: a
default, boring, widely-supported interchange format that most tools simply support, freeing
scientists and engineers from re-solving basic interoperability problems on every project.

Achieving this requires, over time:

- a stable, versioned Core specification (this RFC and its direct successors),
- a growing but disciplined set of extension domain RFCs, developed openly with input from
  space agencies, universities, and industry,
- at least one open-source reference implementation and conformance test suite, so "PTIFF
  compatible" has a testable meaning,
- adoption by tools across the GIS, computer vision, and planetary science communities,
  achieved through genuine technical merit and low integration cost, not mandate.

This vision is aspirational and does not itself impose normative requirements; it exists to
give context to the goals in Section 6 and the governance process in
[`GOVERNANCE.md`](../GOVERNANCE.md).

## 16. Summary

PTIFF is a TIFF/BigTIFF-compatible, open specification for carrying planetary-science
scientific context — camera models, geometry, coordinate reference systems, SPICE-derived
pointing, photometry, stereo, mesh/3D, and AI-derived products — alongside a baseline raster
image, in a single self-contained file. It exists to reduce fragmentation of planetary imagery
metadata across incompatible formats and tools, while explicitly avoiding duplication of the
roles already well served by PDS3/PDS4 archival labels, ISIS internal processing, and GeoTIFF's
Earth-centric georeferencing scope. This RFC defines only the motivation, goals, non-goals, and
design principles of PTIFF; all binary-level and domain-specific normative content is deferred
to the RFCs enumerated in Section 17.

## 17. Open Questions

The following topics are explicitly out of scope for this RFC and MUST be addressed by future,
dedicated RFCs before an implementation can claim full PTIFF conformance:

1. **Binary layout** — precise structure of PTIFF-specific IFDs/sub-IFDs, byte-order handling
   beyond baseline TIFF/BigTIFF rules, and file-level layout conventions for streaming/tiled
   access.
2. **Tag allocation and encoding** — the specific TIFF tag numbers/ranges reserved for PTIFF,
   their data types, and encoding rules (including how private/extension tag ranges are
   partitioned across extension domains).
3. **Camera models** — supported camera model types (pinhole, fisheye, pushbroom/linescan,
   etc.), their parameterization, and distortion model representations.
4. **Coordinate reference systems (CRS)** — how planetary body identifiers, non-Earth
   reference ellipsoids/datums, and map projections are represented, and their relationship to
   existing GeoTIFF CRS conventions.
5. **SPICE integration** — how SPICE kernel references, time systems, and derived
   position/pointing data are represented and how freshness/provenance of SPICE-derived values
   is recorded.
6. **Stereo** — representation of stereo pair relationships, relative pose, and
   disparity/correspondence metadata.
7. **AI layer** — representation of AI/ML-derived products (segmentation, classification,
   uncertainty, embeddings) and their provenance (model identity, version).
8. **Mesh/3D representation** — how mesh or other 3D geometric data is encoded or referenced
   from a PTIFF file.
9. **Compression** — which compression schemes are permitted for baseline and extension data,
   and any planetary-imagery-specific compression considerations (e.g., high dynamic range,
   scientific losslessness requirements).
10. **Streaming and cloud-optimized access** — concrete tiling, overview/pyramid, and
    range-request-friendly layout requirements.
11. **Validation and conformance testing** — the structure of the conformance test suite
    (`conformance/`), conformance levels, and certification/self-declaration process.
12. **Provenance and versioning metadata** — how processing history, software versions, and
    reproducibility metadata are represented across extension domains.
13. **Multi-file relationships** — how related PTIFF files (e.g., a stereo pair, a
    time series, or a mosaic) reference one another, if at all, beyond what is embedded in a
    single file.
14. **Internationalization and units** — canonical unit systems and any text/locale handling
    requirements for human-readable metadata fields.
