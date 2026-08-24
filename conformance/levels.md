# PTIFF Conformance Levels

> Normative. Per RFC-0001 §17.11, the structure of the conformance suite *and the
> conformance levels* are defined here (and in the extension RFCs they reference), not in this
> document's predecessors. These levels are the vocabulary a self-declaration must use.

Levels are cumulative: claiming level *N* implies satisfying every requirement of levels
`0..N`. A level is *earned* only by passing the corresponding executable tests in the matrix
([matrix.md](matrix.md)) and citing the test run as evidence.

Key words "MUST", "SHOULD", "SHOULD NOT", "MAY" in these levels are to be interpreted as
described in RFC 2119 / RFC 8174 (normative text), matching RFC-0001 §10.

---

## Level 0 — Baseline / container

The product of a *PTIFF-unaware* generic TIFF/BigTIFF reader must be able to read the file.

**Requirements (normative):**

- L0.1 The file MUST be parseable as a valid **TIFF** file per the TIFF 6.0 specification, or,
  where BigTIFF is used, as a valid **BigTIFF** file per the Adobe BigTIFF specification.
- L0.2 The file MUST begin with a correct byte-order marker (`II` for little-endian, `MM` for
  big-endian) and a matching magic number for the container being used
  (`42` for classic TIFF, `43` for BigTIFF).
- L0.3 The first IFD offset MUST be valid and every chained IFD MUST be reachable and
  structurally sound (correct entry count, in-bounds field offsets/counts).
- L0.4 A generic reader MUST be able to locate and decode the **baseline image** using only
  standard TIFF/BigTIFF mechanisms (required baseline tags present and consistent:
  `ImageWidth`, `ImageLength`, `BitsPerSample`, `Compression`, `PhotometricInterpretation`,
  `StripOffsets`/`TileOffsets` + `StripByteCounts`/`TileByteCounts`, `RowsPerStrip` or
  `TileWidth`/`TileHeight`).
- L0.5 Supported baseline pixel layouts (grayscale 8/16/32-bit uint and 32-bit float;
  RGB 8/16/32-bit; uncompressed or stripped/tiled) MUST read back byte-for-byte (or, for lossy
  codecs, within the codec's documented tolerance).
- L0.6 A `TiffBackend` that claims Level 0 MUST pass every executable test tagged
  `[conformance][baseline]` in [matrix.md](matrix.md).

**Verification:** `crates/ptiff-core/tests/` (Rust integration tests) — see [matrix.md](matrix.md).

---

## Level 1 — PTIFF Core

Everything from Level 0, plus PTIFF structured metadata encoded with TIFF's private-tag
mechanisms so that generic readers remain unaffected.

**Requirements (normative):**

- L1.1 Level 0 MUST be satisfied.
- L1.2 PTIFF-specific metadata MUST be encoded in private tag ranges / IFD structures already
  permitted by TIFF/BigTIFF (RFC-0001 §11), i.e. no tag or structure that a generic reader
  would misinterpret.
- L1.3 A PTIFF-aware reader MUST be able to round-trip the structured metadata without changing
  Level-0 readability of the same file.
- L1.4 Reserved tag numbers and their encodings MUST follow the (ratified) tag-allocation RFC
  before an implementation may claim Level 1.

**Status:** not yet ratified — blocked on the tag-allocation RFC. Executable tagging begins here
only after that RFC exists. See [matrix.md](matrix.md).

---

## Level 2 — Extension domain

Each entry below is a *separate, independently claimable* level derived from the corresponding
extension RFC. A claim names the domain(s), e.g. "PTIFF Camera".

| Domain | RFC status | Executable suites (future) |
|--------|-----------|---------------------------|
| Camera | not yet written | `conformance/camera/` → `crates/ptiff-core/tests/` |
| CRS | not yet written | `conformance/crs/` → |
| SPICE | not yet written | `conformance/spice/` → |
| Stereo | not yet written | `conformance/stereo/` → |
| Photometry | not yet written | `conformance/photometry/` → |
| Mesh | not yet written | `conformance/mesh/` → |
| AI | not yet written | `conformance/ai/` → |

Each domain level MUST:

- L2.1 Build on Level 1 (PTIFF Core) unless the domain RFC explicitly relaxes it.
- L2.2 Define domain-specific MUSTs as normative text in its RFC, mirrored here as a `levels/`
  entry and executable tests.
- L2.3 Track its own version number (each domain is independently versioned, per
  RFC-0001 §10 "Extension domain").

---

## Level 3 — Full PTIFF

Level 0 + Level 1 + **all** ratified extension domains.

- L3.1 Each component level MUST be earned separately and cited.
- L3.2 "Full PTIFF" is only meaningful once the tag-allocation and at least the current flagship
  domains (camera, CRS) are ratified.

---

## Self-declaration process

An implementation claims a level by committing, in its release notes or a
`CONFORMANCE.md` at the artifact root:

1. The level(s) claimed (e.g. `Level 0`, `Level 1`, `PTIFF Camera`).
2. The exact test run evidence: CI job or local `cargo test` log, commit SHA of the suite source,
   and the platform/toolchain used for the run.
3. Any deviations or permitted divergences documented per requirement ID (L0.x, L1.x, L2.y).
4. The version(s) of the extension RFC(s) asserted.

A claim is a self-declaration: it does not imply third-party certification unless the
implementation states otherwise.
