from .ptiff import ptiff_camera, ptiff_open_path_camera


class Camera:
    """Structured camera calibration for one frame.

    Wraps the C-ABI `ptiff_open_path_camera` result (K, [R|t], P and the
    ISO-8601 timestamp) as a small value object. Matrices are returned as flat
    lists of floats (row-major); the projection P = K * [R|t] is computed in
    the C++ library.

    A `Camera` can be built two ways:
        * `Camera.from_path(path)` -- read the calibration embedded in a PTIFF.
        * `Camera(focal_length_x=..., focal_length_y=..., principal_x=...,`
            `principal_y=..., rotation_w=..., ...)` -- describe a camera from
            known parameters, then persist it when creating an image via
            `Image.create(..., camera=cam)`.
    """

    def __init__(
        self,
        data=None,
        *,
        focal_length_x=0.0,
        focal_length_y=0.0,
        principal_x=0.0,
        principal_y=0.0,
        rotation_w=1.0,
        rotation_x=0.0,
        rotation_y=0.0,
        rotation_z=0.0,
        position_x=0.0,
        position_y=0.0,
        position_z=0.0,
        model="pinhole",
        timestamp="",
    ):
        if data is not None:
            self._d = dict(data)
            return
        self._d = {
            "has_intrinsics": 1,
            "has_extrinsics": 1,
            "focal_length_x": float(focal_length_x),
            "focal_length_y": float(focal_length_y),
            "principal_x": float(principal_x),
            "principal_y": float(principal_y),
            "rotation_w": float(rotation_w),
            "rotation_x": float(rotation_x),
            "rotation_y": float(rotation_y),
            "rotation_z": float(rotation_z),
            "position_x": float(position_x),
            "position_y": float(position_y),
            "position_z": float(position_z),
            "model": model,
            "timestamp": timestamp,
        }

    @classmethod
    def from_path(cls, path):
        _, data = ptiff_open_path_camera(str(path))
        return cls(data)

    @property
    def has_intrinsics(self):
        return bool(self._d["has_intrinsics"])

    @property
    def has_extrinsics(self):
        return bool(self._d["has_extrinsics"])

    @property
    def model(self):
        return self._d.get("model", "pinhole")

    @property
    def focal_length_x(self):
        return self._d["focal_length_x"]

    @property
    def focal_length_y(self):
        return self._d["focal_length_y"]

    @property
    def principal_x(self):
        return self._d["principal_x"]

    @property
    def principal_y(self):
        return self._d["principal_y"]

    @property
    def rotation_w(self):
        return self._d.get("rotation_w", 1.0)

    @property
    def rotation_x(self):
        return self._d.get("rotation_x", 0.0)

    @property
    def rotation_y(self):
        return self._d.get("rotation_y", 0.0)

    @property
    def rotation_z(self):
        return self._d.get("rotation_z", 0.0)

    @property
    def position_x(self):
        return self._d.get("position_x", 0.0)

    @property
    def position_y(self):
        return self._d.get("position_y", 0.0)

    @property
    def position_z(self):
        return self._d.get("position_z", 0.0)

    @property
    def timestamp(self):
        return self._d.get("timestamp", "")

    def intrinsics_matrix(self):
        return list(self._d["intrinsics"])

    def extrinsics_matrix(self):
        return list(self._d["extrinsics"])

    def projection_matrix(self):
        return list(self._d["projection"])

    def to_metadata(self):
        """Returns a SWIG `ptiff_camera` struct ready to persist with
        `Image.create(..., camera=self)`."""
        c = ptiff_camera()
        c.has_intrinsics = int(self.has_intrinsics)
        c.focal_length_x = float(self.focal_length_x)
        c.focal_length_y = float(self.focal_length_y)
        c.principal_x = float(self.principal_x)
        c.principal_y = float(self.principal_y)
        c.has_extrinsics = int(self.has_extrinsics)
        c.rotation_w = float(self.rotation_w)
        c.rotation_x = float(self.rotation_x)
        c.rotation_y = float(self.rotation_y)
        c.rotation_z = float(self.rotation_z)
        c.position_x = float(self.position_x)
        c.position_y = float(self.position_y)
        c.position_z = float(self.position_z)
        c.timestamp = self.timestamp
        return c
