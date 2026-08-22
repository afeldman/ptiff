# RFC-0014: Multi-File Relationships

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Multi-File domain scope per RFC-0001 §17 item 13. |

---

## 1. Abstract

This RFC reserves the **Multi-File Relationships** domain of PTIFF: how related PTIFF files —
e.g. a stereo pair, a time series, or a mosaic — reference one another, if at all, beyond what
is embedded in a single file. It resolves RFC-0001 §17 item 13.

This RFC is currently **conceptual / reserved**: no reference mechanism is defined yet. A
future normative version specifies how one PTIFF file may point to peer files.

---

## 2. Status and relationship to other domains

Several PTIFF extension domains anticipate multi-file relationships:

- **Stereo (RFC-0006)** — a stereo pair MAY be distributed across two files; the pair linkage
  is a multi-file concern.
- **Mesh / 3D (RFC-0008)** — a mesh MAY be stored as a companion resource referenced from a
  raster PTIFF; the reference is a multi-file concern.
- **Time series / mosaics** — sequences of related PTIFF files with no single-file
  relationship embedded today.

Within a single TIFF file, multiple IFDs already model co-located related images (e.g. COG
pyramids). Multi-file extends this notion across physical files.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Multi-File RFC SHOULD specify:

- how a related file is **referenced** (path, resolution, MIME / format hints),
- a **relationship vocabulary** (stereo-pair role, time-sequence neighbor, mosaic tile, mesh
  companion, …),
- **consistency / integrity** handling when a referenced peer is missing or changed,
- alignment with the Provenance domain (RFC-0009) where relationships are recorded.

---

## 4. Conformance

There are currently no normative multi-file requirements. A PTIFF reader MUST treat any
reference mechanism it does not recognize as informational and MUST NOT fail a file because a
referenced peer is absent.

---

## 5. References

- `RFC-0001-Core.md` — §17 item 13.
- `RFC-0006` — Stereo domain.
- `RFC-0008` — Mesh / 3D domain.
- `RFC-0009` — Provenance domain.
