#!/usr/bin/env python3
"""Camera calibration + metadata test for the SWIG-generated ptiff module.

Exercises the structured camera calibration (Professor requirement 2/3):
`ptiff_open_path_camera` returns the K / [R|t] / P matrices and the ISO-8601
timestamp computed in the C++ library, and the idiomatic Python object layer
(`Image`, `Camera`, `Tile`) wraps them as native classes.

Run against a configured libptiff_c (see bindings/swig/#python) with the
interop fixture available at scripts/samples.
"""

import os
import sys

import ptiff

# Fixture written by scripts/samples interop generator: intrinsics only.
FIXTURE = os.path.realpath(
    os.path.join(
        os.path.dirname(__file__),
        "..",
        "..",
        "..",
        "scripts",
        "samples",
        "ptiff_interop_fixture.tif",
    )
)


def approx(a: float, b: float, tol: float = 1e-9) -> bool:
    return abs(a - b) <= tol


def main() -> int:
    assert os.path.exists(FIXTURE), "interop fixture missing: {FIXTURE}"

    # ---- Structured C-ABI camera (ptiff_open_path_camera -> dict) ----
    rc, cam = ptiff.ptiff_open_path_camera(FIXTURE)
    assert rc == 0, f"ptiff_open_path_camera returned rc={rc}"
    assert isinstance(cam, dict), f"expected a dict, got {type(cam)!r}"
    assert cam["has_intrinsics"] == 1, "fixture must carry intrinsics"
    assert approx(cam["focal_length_x"], 700.0)
    assert approx(cam["focal_length_y"], 700.0)
    assert approx(cam["principal_x"], 64.0)
    assert approx(cam["principal_y"], 64.0)
    K = cam["intrinsics"]
    assert len(K) == 9
    assert approx(K[0], 700.0)  # fx
    assert approx(K[1], 0.0)
    assert approx(K[2], 64.0)  # cx
    assert approx(K[4], 700.0)  # fy
    assert approx(K[5], 64.0)  # cy
    assert approx(K[8], 1.0)

    # Fixture has no extrinsics fields -> has_extrinsics must be 0.
    assert cam["has_extrinsics"] == 0

    # ---- Idiomatic object layer (class-image/camera/tile) ----
    with ptiff.Image.open(FIXTURE) as img:
        assert img.width == 128
        assert img.height == 128
        assert img.channel_count == 1

        camera = img.camera
        assert isinstance(camera, ptiff.Camera)
        assert approx(camera.focal_length_x, 700.0)
        K2 = camera.intrinsics_matrix()
        assert approx(K2[0], 700.0)
        P = camera.projection_matrix()
        # K * [R|t] with identity pose and t=0 collapses onto K.
        assert approx(P[0], 700.0)
        assert approx(P[2], 64.0)
        assert approx(P[3], 0.0)  # translation x = 0 (no extrinsics)
        assert approx(P[10], 1.0)  # K[2][2] = 1

        # A tile reads back as a Tile value object.
        tile = img.read_tile(0, 0)
        assert isinstance(tile, ptiff.Tile)
        assert tile.column == 0 and tile.row == 0
        assert len(tile.data) == tile.byte_size

    # ---- Write side: persist a camera as metadata, then read it back ----
    OUT = os.path.join(os.path.dirname(__file__), "camera_roundtrip_python.tif")
    if os.path.exists(OUT):
        os.remove(OUT)

    cam = ptiff.Camera(
        focal_length_x=700.0,
        focal_length_y=715.0,
        principal_x=32.0,
        principal_y=24.0,
        rotation_w=1.0,
        position_x=1.0,
        position_y=2.0,
        position_z=3.0,
        timestamp="2026-08-21T12:34:56.000Z",
    )

    with ptiff.Image.create(
        OUT, 32, 32, pixel_type=0, tile_width=16, tile_height=16, camera=cam
    ) as img_out:
        img_out.write_tile(0, 0, bytes(16 * 16))
        img_out.write_tile(1, 0, bytes(16 * 16))
        img_out.write_tile(0, 1, bytes(16 * 16))
        img_out.write_tile(1, 1, bytes(16 * 16))

    # Read the written file back through the object layer.
    with ptiff.Image.open(OUT) as img2:
        cam2 = img2.camera
        assert isinstance(cam2, ptiff.Camera)
        assert approx(cam2.focal_length_x, 700.0)
        assert approx(cam2.focal_length_y, 715.0)
        assert approx(cam2.principal_x, 32.0)
        assert approx(cam2.principal_y, 24.0)
        assert cam2.has_intrinsics == 1, "intrinsics must round-trip"
        # Extrinsics: rotation quaternion + world position must be persisted.
        assert approx(cam2.rotation_w, 1.0)
        assert approx(cam2.position_x, 1.0)
        assert approx(cam2.position_y, 2.0)
        assert approx(cam2.position_z, 3.0)
        assert cam2.timestamp == "2026-08-21T12:34:56.000Z"
        # Projection P = K * [R|t] (identity R): P[0,3] = fx*tx + cx*tz.
        P2 = cam2.projection_matrix()
        assert approx(P2[0], 700.0)
        assert approx(P2[3], 700.0 * 1.0 + 32.0 * 3.0)  # fx*tx + cx*tz
        assert approx(P2[7], 715.0 * 2.0 + 24.0 * 3.0)  # fy*ty + cy*tz
        assert approx(P2[11], 3.0)  # tz
    os.remove(OUT)

    print("test_camera: OK (structured camera + object layer + write roundtrip)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
