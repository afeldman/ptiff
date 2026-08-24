"""Camera calibration write+read (feature parity with SWIG ptiff.camera)."""

import numpy as np
import ptiff_pyo3 as ptiff


def test_camera_construct_and_read(tmp_path):
    cam = ptiff.Camera(
        focal_length_x=900.0,
        focal_length_y=905.0,
        principal_x=320.0,
        principal_y=240.0,
        rotation_w=1.0,
        position_z=50.0,
        timestamp="2025-01-01T00:00:00Z",
    )
    assert cam.model == "pinhole"
    assert cam.has_intrinsics is True
    assert cam.has_extrinsics is True

    path = str(tmp_path / "cam.tif")
    sink = ptiff.create_image(
        path,
        width=64,
        height=32,
        tile_width=64,
        tile_height=32,
        camera=cam,
    )
    data = np.arange(64 * 32, dtype=np.uint8) % 251
    sink.write_tile(0, 0, data.tobytes())
    sink.close()

    doc = ptiff.open(path)
    c2 = doc.camera(0)
    assert c2.has_intrinsics
    assert abs(c2.focal_length_x - 900.0) < 1e-9
    assert abs(c2.focal_length_y - 905.0) < 1e-9
    assert abs(c2.principal_x - 320.0) < 1e-9
    assert abs(c2.position_z - 50.0) < 1e-9
    assert c2.timestamp == "2025-01-01T00:00:00Z"

    # Matrices are readable on a camera read back from the file.
    intr = c2.intrinsics_matrix()
    assert len(intr) == 9
    assert abs(intr[0] - 900.0) < 1e-9  # K[0][0] = fx
    assert abs(intr[2] - 320.0) < 1e-9  # K[0][2] = cx


def test_camera_absent_is_empty(tmp_path):
    path = str(tmp_path / "nocam.tif")
    sink = ptiff.create_image(path, width=64, height=32)
    sink.write_tile(0, 0, bytes(sink.tile_byte_size))
    sink.close()
    cam = ptiff.open(path).camera(0)
    assert cam.has_intrinsics is False
    assert cam.has_extrinsics is False
