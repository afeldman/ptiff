# PTIFF Specification — Stereo Domain

**Status:** Draft — conceptual; no dedicated private tag allocated yet
**Storage notation:** future `ptiff.stereo.*` (concept)
**Normative basis:** `RFC-0001-Core.md` (§9 UC2, §17 item 6).

---

## 1. Purpose

The Stereo domain represents **stereo image relationships**: how two or more images form a
stereo pair, the relative pose between the cameras, and the disparity / correspondence
metadata needed to reconstruct depth. RFC-0001's concrete use case:

> **UC2 — Stereo pair processing.** Two or more PTIFF images from a stereo rig carry enough
> calibration to permit a stereo-matching or photometric-correction algorithm to derive
> disparity or depth without additional configuration. — RFC-0001 §9

---

## 2. Status and current implementation

The Stereo domain is **conceptual / reserved**: no dedicated private tag is currently
allocated and no `ptiff.stereo.*` field schema is yet defined. The material a future Stereo
extension RFC must specify is captured in RFC-0001 §17 item 6:

> **Stereo** — representation of stereo pair relationships, relative pose, and
> disparity/correspondence metadata. — RFC-0001 §17 item 6

The foundational building blocks already exist and a future RFC can reference them:

- **Per-view camera calibration** — each view's intrinsics/extrinsics are fully specified by
  the Camera domain (`ptiff.camera.*`, tag 65002). Two views of a stereo rig therefore already
  carry their **camera intrinsics** and **extrinsic pose**; the *relative* pose between the two
  cameras is derivable from their individual extrinsics.
- **Layer products** — depth / disparity rasters are per-pixel derived layers and can be
  carried today on the Scientific-Layers tag (65004) using `ptiff.layers.*` keys.

Stereo-derived multi-file relationships SHOULD also consult RFC-0001 §17 item 13
("Multi-file relationships") when a stereo pair is stored across two files.

---

## 3. Planned scope (reserved for a future extension RFC)

When the Stereo extension RFC is written it SHOULD specify:

- how a stereo **pair / rig relationship** is named and linked (pair id, left/right or
  index roles),
- the **relative pose** representation (rotation + translation between views) and its
  relationship to each view's extrinsics,
- **disparity / correspondence** layer schemas (dtype, unit, epipolar conventions),
- alignment with the Scientific-Layers tag (65004) and any future multi-file linkage.

---

## 4. Conformance

There are currently no normative stereo requirements beyond the generic container rules. A
PTIFF reader MUST tolerate a file with no stereo metadata.

---

## 5. References

- [`../camera/calibration.md`](../camera/calibration.md) — per-view intrinsics/extrinsics.
- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65004 and record rules.
- `RFC-0001-Core.md` — §9 UC2, §17 item 6 and item 13.
