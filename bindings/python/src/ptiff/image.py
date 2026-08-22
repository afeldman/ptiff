import contextlib

from .camera import Camera
from .ptiff import (
    ptiff_image_descriptor,
    ptiff_sink_close,
    ptiff_sink_create,
    ptiff_sink_create_camera,
    ptiff_sink_tile_byte_size,
    ptiff_sink_tile_columns,
    ptiff_sink_tile_rows,
    ptiff_sink_write_tile,
    ptiff_source_close,
    ptiff_source_descriptor,
    ptiff_source_open,
    ptiff_source_read_tile,
    ptiff_source_tile_byte_size,
    ptiff_source_tile_columns,
    ptiff_source_tile_rows,
    ptiff_tile_info,
)
from .tile import Tile


class Image:
    """A PTIFF image: metadata plus tile access.

    Use the classmethods `Image.open(path)` (read a TIFF/BigTIFF) and
    `Image.create(path, width, height, pixel_type=...)` (write a new one).
    Reading exposes the structured camera calibration via `img.camera` and the
    raw tile bytes via `img.read_tile(col, row)`.
    """

    def __init__(self, path, handle=None, camera=None):
        self.path = path
        self._handle = handle
        self._sink = None
        self._camera = camera if camera is not None else Camera.from_path(path)
        self._desc = None

    @classmethod
    def open(cls, path):
        path = str(path)
        result = ptiff_source_open(path)
        # SWIG surfaces the optional err_out as a second list element; the
        # handle is always the first.
        handle = result[0] if isinstance(result, (list | tuple)) else result
        if not handle:
            raise OSError(f"ptiff_source_open failed for {path!r}")
        return cls(path, handle=handle)

    @classmethod
    def create(
        cls,
        path,
        width,
        height,
        pixel_type=0,
        tile_width=0,
        tile_height=0,
        channel_count=1,
        camera=None,
    ):
        path = str(path)
        desc = ptiff_image_descriptor()
        desc.width = int(width)
        desc.height = int(height)
        desc.pixel_type = int(pixel_type)
        desc.channel_count = int(channel_count)
        if tile_width > 0 and tile_height > 0:
            ti = ptiff_tile_info()
            ti.tile_width = int(tile_width)
            ti.tile_height = int(tile_height)
            desc.tile_info = ti
            desc.has_tile_info = 1
        if camera is not None:
            cam = camera.to_metadata() if isinstance(camera, Camera) else camera
            sink = ptiff_sink_create_camera(path, desc, cam)
            if not sink:
                raise OSError(f"ptiff_sink_create_camera failed for {path!r}")
        else:
            sink = ptiff_sink_create(path, desc)
            if not sink:
                raise OSError(f"ptiff_sink_create failed for {path!r}")
        obj = cls(path)
        obj._sink = sink
        obj._desc = desc
        obj._camera = camera
        return obj

    # -- metadata ------------------------------------------------------------
    def _descriptor(self):
        if self._desc is None:
            self._desc = ptiff_image_descriptor()
            ptiff_source_descriptor(self._handle, self._desc)
        return self._desc

    @property
    def descriptor(self):
        return self._descriptor()

    @property
    def width(self):
        return self._descriptor().width

    @property
    def height(self):
        return self._descriptor().height

    @property
    def channel_count(self):
        return self._descriptor().channel_count

    @property
    def pixel_type(self):
        return self._descriptor().pixel_type

    @property
    def camera(self):
        return self._camera

    @property
    def tile_columns(self):
        if self._handle:
            return int(ptiff_source_tile_columns(self._handle))
        if self._sink:
            return int(ptiff_sink_tile_columns(self._sink))
        return 0

    @property
    def tile_rows(self):
        if self._handle:
            return int(ptiff_source_tile_rows(self._handle))
        if self._sink:
            return int(ptiff_sink_tile_rows(self._sink))
        return 0

    @property
    def tile_byte_size(self):
        if self._handle:
            return int(ptiff_source_tile_byte_size(self._handle))
        if self._sink:
            return int(ptiff_sink_tile_byte_size(self._sink))
        return 0

    def read_tile(self, column, row):
        if self._handle is None:
            raise OSError("image not open for reading")
        nbytes = self.tile_byte_size
        buf = bytearray(nbytes)
        ptiff_source_read_tile(self._handle, int(column), int(row), buf)
        return Tile(int(column), int(row), bytes(buf), byte_size=nbytes)

    def write_tile(self, column, row, data):
        if self._sink is None:
            raise OSError("image not open for writing")
        buf = bytearray(data)
        ptiff_sink_write_tile(self._sink, int(column), int(row), buf)

    def close(self):
        if self._handle is not None:
            with contextlib.suppress(Exception):
                ptiff_source_close(self._handle)
            self._handle = None
        if self._sink is not None:
            with contextlib.suppress(Exception):
                ptiff_sink_close(self._sink)
            self._sink = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False

    def __repr__(self):
        return f"<ptiff.Image path={self.path!r} {self.width}x{self.height}>"
