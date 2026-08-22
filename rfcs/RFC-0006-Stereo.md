# RFC-0006: Stereo Pair Relationships & Disparity

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-0003 (Camera), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Stereo domain scope per RFC-0001 §17 item 6; no dedicated tag or `ptiff.stereo.*` schema is allocated yet. |

---

## 1. Abstract

This RFC reserves the **Stereo domain** of PTIFF: how two or more images form a stereo pair,
the relative pose between the cameras, and the disparity / correspondence metadata needed to
reconstruct depth. RFC-0001's concrete use case:

> **UC2 — Stereo pair processing.** Two or more PTIFF images from a stereo rig carry enough
> calibration to permit a stereo-matching or photometric-correction algorithm to derive
> disparity or depth without additional configuration.
> — RFC-0001 §9

This RFC is currently **conceptual / reserved**: no dedicated private tag is allocated and no
`ptiff.stereo.*` field schema is defined yet. It records the required scope that a future,
normative Stereo extension RFC MUST specify, resolving RFC-0001 §17 item 6.

---

## 2. Status and existing building blocks

The Stereo domain is reserved. The foundational pieces needed by a future RFC already exist:

- **Per-view camera calibration** — each view's intrinsics / extrinsics are fully specified by
  the Camera domain (RFC-0003, tag 65002). The *relative* pose between two cameras is derivable
  from their individual extrinsics.
- **Layer products** — depth / disparity rasters are per-pixel derived layers and can be carried
  today on the Scientific-Layers tag (65004) using `ptiff.layers.*` keys.

Multi-file stereo distributions SHOULD also consult RFC-0001 §17 item 13 ("Multi-file
relationships").

---

## 3. Planned scope (required of a future extension RFC)

A future normative Stereo RFC SHOULD specify:

- how a stereo **pair / rig relationship** is named and linked (pair id, left/right or index
  roles),
- the **relative pose** representation (rotation + translation between views) and its
  relationship to each view's extrinsics,
- **disparity / correspondence** layer schemas (dtype, unit, epipolar conventions),
- alignment with the Scientific-Layers tag (65004) and any future multi-file linkage.

---

## 4. Conformance

There are currently no normative stereo requirements beyond the generic container rules of
RFC-7002. A PTIFF reader MUST tolerate a file with no stereo metadata.

---

## 5. References

- `RFC-0001-Core.md` — §9 UC2, §17 items 6 and 13.
- `RFC-0003` — per-view intrinsics / extrinsics (tag 65002).
- `RFC-7002` — payload codec, tags 65002 / 65004.
- Reference implementation: `specification/stereo/pair-metadata.md`.
