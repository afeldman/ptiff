"""Metadata wrapper over the SWIG low-level metadata surface.

The C ABI reads a file's metadata without materialising a live image handle:
`ptiff_open_path` (core descriptor: width/height/pixel type/channel count/tile
info) and `ptiff_open_path_fields` (the flattened PTIFF extension fields,
e.g. ``ptiff.camera.model``). This module wraps both as a small read-only
``Metadata`` value object, uniform with `Camera`/`Image`/`Tile`.
"""

from .ptiff import (
    ptiff_image_descriptor,
    ptiff_open_path,
    ptiff_open_path_fields,
)

_PIXEL_TYPE_NAMES = {
    0: "UInt8",
    1: "UInt16",
    2: "UInt32",
    3: "Float32",
    4: "Float64",
}


class Metadata:
    """Read-only metadata of a single image file.

    Built from the file itself (no live handle is kept open): the core
    descriptor and the flattened PTIFF extension fields.
    """

    def __init__(self, path):
        path = str(path)
        self._path = path

        desc = ptiff_image_descriptor()
        rc = ptiff_open_path(path, desc)
        if rc != 0:
            raise OSError(f"ptiff_open_path failed for {path!r} (rc={rc})")
        self._desc = desc

        self._fields = self._read_fields(path)

    @classmethod
    def _read_fields(cls, path):
        # typemap: ptiff_open_path_fields(path) -> [rc, [(key, value), ...]]
        result = ptiff_open_path_fields(path)
        rc = result[0] if isinstance(result, (list | tuple)) else result
        if rc != 0:
            raise OSError(f"ptiff_open_path_fields failed for {path!r} (rc={rc})")
        pairs = result[1] if isinstance(result, (list | tuple)) else []
        return dict(pairs)

    @property
    def path(self):
        return self._path

    @property
    def width(self):
        return self._desc.width

    @property
    def height(self):
        return self._desc.height

    @property
    def channel_count(self):
        return self._desc.channel_count

    @property
    def pixel_type(self):
        return self._desc.pixel_type

    @property
    def pixel_type_name(self):
        return _PIXEL_TYPE_NAMES.get(self._desc.pixel_type, "Unknown")

    @property
    def tile_width(self):
        return self._desc.tile_info.tile_width if self._desc.has_tile_info else 0

    @property
    def tile_height(self):
        return self._desc.tile_info.tile_height if self._desc.has_tile_info else 0

    @property
    def fields(self):
        """The flattened PTIFF extension fields as an ordered key->value dict."""
        return dict(self._fields)

    def field(self, key, default=None):
        """Return a single extension field value, or `default` when absent."""
        return self._fields.get(key, default)

    def __repr__(self):
        return (
            f"<ptiff.Metadata path={self.path!r} {self.width}x{self.height} "
            f"px={self.pixel_type_name} ch={self.channel_count}>"
        )
