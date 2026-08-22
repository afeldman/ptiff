"""ptiff public API.

The package is split into two layers:

* ``ptiff.ptiff`` -- the raw SWIG-generated binding to the libptiff C ABI.
    SWIG is only asked to translate the C functions/structs/constants; it does
    not build any idiomatic object model (``Camera``/``Image``/``Tile``).
* ``ptiff.camera`` / ``ptiff.image`` / ``ptiff.tile`` / ``ptiff.logger`` /
    ``ptiff.metadata`` -- the idiomatic Python object layer: thin wrapper
    classes around the SWIG low-level surface.

Everything is re-exported on ``ptiff`` so ``import ptiff`` gives both the
raw functions (``ptiff.ptiff_source_open(...)``) and the object layer
(``ptiff.Image.open(...)``) in one namespace.
"""

# 1) Raw SWIG low-level surface: ptiff_* free functions/structs and PTIFF_*
#    constants land directly on the package, so existing tests that reach
#    `ptiff.ptiff_source_open`, `ptiff.ptiff_runtime_version`,
#    `ptiff.PTIFF_LOG_ERROR`, ... keep resolving.
# 2) Idiomatic object layer on top of the low-level surface.
from .camera import Camera
from .image import Image
from .logger import Logger
from .metadata import Metadata
from .ptiff import *  # noqa: F403
from .tile import Tile

__all__ = [
    "Camera",
    "Image",
    "Logger",
    "Metadata",
    "Tile",
]
