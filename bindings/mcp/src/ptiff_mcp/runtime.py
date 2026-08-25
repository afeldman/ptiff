"""PTIFF runtime access layer for the MCP server.

Thin, dependency-light wrapper over the PyO3 ``ptiff_pyo3`` object layer
(Document / Metadata / Camera / Image / Sink). The MCP consumes only the
promoted Python objects and the metadata they expose (structured camera
calibration and PTIFF extension fields included).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

import ptiff_pyo3 as _ptiff

# Pixel type names (mirror the C ABI PTIFF_PIXEL_* / PyO3 PTIFF_PIXEL_* values).
# The MCP exposes lowercase names ("uint8") for LLM ergonomics; the object
# layer exposes the same int codes plus a human name (Metadata.pixel_type_name
# / Image.pixel_type).
_PIXEL_NAMES: dict[int, str] = {
    _ptiff.PTIFF_PIXEL_UINT8: "uint8",
    _ptiff.PTIFF_PIXEL_UINT16: "uint16",
    _ptiff.PTIFF_PIXEL_UINT32: "uint32",
    _ptiff.PTIFF_PIXEL_FLOAT32: "float32",
    _ptiff.PTIFF_PIXEL_FLOAT64: "float64",
}

_COMPRESSION_NAMES: dict[int, str] = {
    _ptiff.PTIFF_COMPRESSION_NONE: "none",
    _ptiff.PTIFF_COMPRESSION_LZW: "lzw",
    _ptiff.PTIFF_COMPRESSION_DEFLATE: "deflate",
    _ptiff.PTIFF_COMPRESSION_JPEG: "jpeg",
}

# Bytes per sample for each PTIFF_PIXEL_* code (derived, not read from a
# getter — the PyO3 Image exposes tile_width/tile_height but the byte size is
# derived from pixel type × channels).
_PIXEL_ITEM_SIZE: dict[int, int] = {
    _ptiff.PTIFF_PIXEL_UINT8: 1,
    _ptiff.PTIFF_PIXEL_UINT16: 2,
    _ptiff.PTIFF_PIXEL_UINT32: 4,
    _ptiff.PTIFF_PIXEL_FLOAT32: 4,
    _ptiff.PTIFF_PIXEL_FLOAT64: 8,
}

# Number of samples we ever let a tile statistic produce, to keep LLM context
# small. Sampling buckets are logarithmic-ish so wide histograms still compress.
MAX_SAMPLE_BUCKETS = 64


class PtiffError(RuntimeError):
    """Raised when a ptiff operation fails.

    ``ptiff_pyo3`` raises ordinary Python exceptions on failure (there are no
    C-style error codes any more); this type wraps them in a readable MCP
    message.
    """

    def __init__(self, message: str, code: int | None = None) -> None:
        super().__init__(message)
        self.code = code


@dataclass
class Metadata:
    """Primary-image metadata for a file (LLM-friendly, additive view).

    ``fields`` are the flattened PTIFF extension fields (``ptiff.*`` keys);
    ``camera`` is the structured camera calibration (K/[R|t]/P, timestamp)
    when the file carries one, else ``None``.
    """

    path: str
    width: int
    height: int
    pixel_type: str
    channel_count: int
    tile_width: int | None
    tile_height: int | None
    compression: str | None
    tile_columns: int | None
    tile_rows: int | None
    tile_byte_size: int | None
    fields: dict[str, str] = field(default_factory=dict)
    camera: dict[str, Any] | None = None


def get_version() -> dict[str, str]:
    return {
        "runtime": _version_str(_ptiff.runtime_version()),
        "compile_time": _version_str(_ptiff.compile_time_version()),
    }


def _version_str(v: Any) -> str:
    """Format a (major, minor, patch) tuple/list as ``major.minor.patch``."""
    try:
        major, minor, patch = v
    except (TypeError, ValueError):
        major = minor = patch = 0
    return f"{int(major)}.{int(minor)}.{int(patch)}"


def list_backends() -> list[str]:
    # ``ptiff_pyo3.backend_names()`` returns one comma-space-joined string
    # (e.g. ``"tiff, memory"``), matching the C ABI's ``ptiff_backend_names()``.
    names = _ptiff.backend_names()
    if not names:
        return []
    return [n.strip() for n in str(names).split(",") if n.strip()]


def _camera_to_dict(camera: Any) -> dict[str, Any] | None:
    """Flatten the object-layer Camera into a serializable dict.

    The object layer returns camera matrices as flat row-major lists
    (intrinsics 9, extrinsics 12, projection 12) via its *_matrix() methods.
    """
    try:
        if camera is None:
            return None
        has_i = bool(getattr(camera, "has_intrinsics", False))
        has_e = bool(getattr(camera, "has_extrinsics", False))
        if not has_i and not has_e:
            return None
        d: dict[str, Any] = {
            "has_intrinsics": has_i,
            "has_extrinsics": has_e,
            "model": getattr(camera, "model", None),
            "timestamp": getattr(camera, "timestamp", None),
        }
        if has_i:
            d["focal_length_x"] = float(getattr(camera, "focal_length_x", 0.0))
            d["focal_length_y"] = float(getattr(camera, "focal_length_y", 0.0))
            d["principal_x"] = float(getattr(camera, "principal_x", 0.0))
            d["principal_y"] = float(getattr(camera, "principal_y", 0.0))
            try:
                d["intrinsics_matrix"] = list(camera.intrinsics_matrix())
            except Exception:  # noqa: BLE001 -- fall back to raw list
                d["intrinsics_matrix"] = list(getattr(camera, "intrinsics", []))
        if has_e:
            try:
                d["extrinsics_matrix"] = list(camera.extrinsics_matrix())
            except Exception:  # noqa: BLE001 -- fall back to raw list
                d["extrinsics_matrix"] = list(getattr(camera, "extrinsics", []))
            try:
                d["projection_matrix"] = list(camera.projection_matrix())
            except Exception:  # noqa: BLE001 -- fall back to raw list
                d["projection_matrix"] = list(getattr(camera, "projection", []))
        return d
    except Exception:  # noqa: BLE001 -- a camera is optional; degrade gracefully
        return None


def read_metadata(path: str) -> Metadata:
    """Read primary-image metadata (core descriptor, extension fields, camera).

    Uses the PyO3 ``Document`` object (``ptiff_pyo3.open``): ``doc.metadata()``
    for the core descriptor + extension fields, ``doc.image(0)`` for the tile
    grid geometry, and ``doc.camera(0)`` for structured calibration when present.
    """
    path = str(path)
    try:
        with _ptiff.open(path) as doc:
            md = doc.metadata()

            pixel = str(md.pixel_type_name)
            fields = {str(k): str(v) for k, v in (md.fields or {}).items()}

            tw = int(md.tile_width) if md.tile_width else None
            th = int(md.tile_height) if md.tile_height else None

            # Tile grid geometry + byte size come off the primary Image.
            cols = rows = bs = None
            try:
                img = doc.image(0)
                cols, rows = _tile_grid(img)
                cols = cols or None
                rows = rows or None
                bs = tile_byte_size(img)
            except Exception:  # noqa: BLE001 -- grid geometry is best-effort
                pass

            camera: dict[str, Any] | None = None
            try:
                cam = doc.camera(0)
                if cam is not None:
                    camera = _camera_to_dict(cam)
            except Exception:  # noqa: BLE001 -- not all files carry calibration
                camera = None

        return Metadata(
            path=path,
            width=int(md.width),
            height=int(md.height),
            pixel_type=pixel,
            channel_count=int(md.channel_count),
            tile_width=tw,
            tile_height=th,
            compression=None,  # not exposed by the PyO3 Metadata; kept optional
            tile_columns=cols,
            tile_rows=rows,
            tile_byte_size=bs,
            fields=fields,
            camera=camera,
        )
    except PtiffError:
        raise
    except Exception as exc:
        raise PtiffError(f"read metadata {path}: {exc}") from exc


def _tile_grid(img: Any) -> tuple[int, int]:
    """Return (tile_columns, tile_rows) for a PyO3 Image.

    The PyO3 ``Image`` exposes the tile *size* (``tile_width``/``tile_height``)
    but not the grid dimensions; the grid is derived from the image dims:
        tile_columns = ceil(width / tile_width),
        tile_rows = ceil(height / tile_height).
    """
    W = _ceil_div(int(img.width), int(img.tile_width)) if int(img.tile_width) else 0
    H = _ceil_div(int(img.height), int(img.tile_height)) if int(img.tile_height) else 0
    return W, H


def tile_byte_size(img: Any) -> int:
    """Byte size of one uniform tile, derived from the open Image's layout."""
    tw = th = ch = code = 0
    try:
        tw = int(img.tile_width)
        th = int(img.tile_height)
        ch = int(img.channel_count)
        # PyO3 exposes ``Image.pixel_type`` as a lowercase name ("uint8") —
        # map it back to the PTIFF_PIXEL_* code (fall back to 0/uint8).
        pixel = str(img.pixel_type).lower()
        code = next((k for k, v in _PIXEL_NAMES.items() if v == pixel), 0)
    except Exception:  # noqa: BLE001
        code = 0
    itemsize = _PIXEL_ITEM_SIZE.get(code, 1)
    return max(0, tw * th * ch * itemsize)


def metadata_to_dict(md: Metadata) -> dict[str, Any]:
    d: dict[str, Any] = {
        "path": md.path,
        "width": md.width,
        "height": md.height,
        "pixel_type": md.pixel_type,
        "channel_count": md.channel_count,
        "tile_width": md.tile_width,
        "tile_height": md.tile_height,
        "compression": md.compression,
        "tile_columns": md.tile_columns,
        "tile_rows": md.tile_rows,
        "tile_byte_size": md.tile_byte_size,
    }
    if md.fields:
        d["fields"] = md.fields
    if md.camera is not None:
        d["camera"] = md.camera
    return d


def read_tile(path: str, column: int, row: int) -> dict[str, Any]:
    """Read one tile/strip and return compact statistics (never raw pixels)."""
    with _ptiff.open(path) as doc:
        img = doc.image(0)
        cols, rows_i = _tile_grid(img)
        if not (0 <= column < cols) or not (0 <= row < rows_i):
            raise PtiffError(
                f"tile ({column},{row}) out of range 0..{cols - 1}, 0..{rows_i - 1}"
            )
        arr = img.read_tile(column, row)
        md = _metadata_for(img, tile_byte_size(img), str(path))
        # The tile arrives as a numpy.ndarray (tile_height, tile_width, channels).
        return _analyze_samples(arr.tobytes(), md)


def _metadata_for(img: Any, tile_byte_size: int, path: str = "") -> Metadata:
    """Build an MCP Metadata from an open Image (read side).

    The PyO3 ``Image`` exposes width/height/channels + a ``pixel_type`` *name*
    (lowercase, e.g. "uint8") and the tile *size* (``tile_width``/
    ``tile_height``); the grid dimensions and byte size are derived.
    """
    pixel = str(img.pixel_type)
    cols, rows_i = _tile_grid(img)
    return Metadata(
        path=path,
        width=int(img.width),
        height=int(img.height),
        pixel_type=pixel,
        channel_count=int(img.channel_count),
        tile_width=int(img.tile_width),
        tile_height=int(img.tile_height),
        compression=None,
        tile_columns=cols,
        tile_rows=rows_i,
        tile_byte_size=tile_byte_size,
    )


def _analyze_samples(data: bytes, md: Metadata) -> dict[str, Any]:
    """Produce compact statistics from raw tile bytes.

    Returns min/max/mean/count plus a small log-binned histogram, bounded by
    MAX_SAMPLE_BUCKETS so the response stays LLM-friendly.
    """
    import struct  # local to keep module import cheap

    pixel = md.pixel_type
    ch = max(1, md.channel_count)
    fmt, width = {
        "uint8": ("B", 1),
        "uint16": ("H", 2),
        "uint32": ("I", 4),
        "float32": ("f", 4),
        "float64": ("d", 8),
    }.get(pixel, (None, None))
    if fmt is None:
        return {"pixel_type": pixel, "error": f"unsupported pixel type {pixel}"}

    n_items = len(data) // (width * ch)
    if n_items == 0:
        return {
            "pixel_type": pixel,
            "channel_count": ch,
            "samples": 0,
            "note": "empty tile",
        }

    stats = []
    for c in range(ch):
        # Collect this channel's samples (all pixels in the tile, channel c).
        samples = []
        stride = width * ch
        off = c * width
        for i in range(n_items):
            pos = off + i * stride
            samples.append(struct.unpack_from(fmt, data, pos)[0])
        stats.append(_channel_stats(samples))
    return {
        "pixel_type": pixel,
        "channel_count": ch,
        "sample_count": n_items,
        "channels": stats,
    }


def _channel_stats(values: list[float | int]) -> dict[str, Any]:
    n = len(values)
    lo = min(values)
    hi = max(values)
    mean = sum(values) / n
    # Log-binned histogram: bucket boundaries grow exponentially so wide
    # ranges still compress into <= MAX_SAMPLE_BUCKETS buckets.
    from math import floor as _floor
    from math import log as _log

    buckets: dict[int, int] = {}
    if hi > lo:
        log_lo = _log(lo + 1.0) if lo >= 0 else _log(max(lo, 1e-300))
        span = (
            _log(hi + 1.0) - log_lo
            if hi >= 0
            else _log(abs(hi) + 1.0) - _log(abs(lo) + 1.0)
        )
        for v in values:
            if hi > lo and span > 0:
                rel = (v - lo) / (hi - lo)
                rel = max(0.0, min(1.0, rel))
                b = int(_floor(rel * 63))
                buckets[b] = buckets.get(b, 0) + 1
        hist = [
            [int(lo + (hi - lo) * (b + 0.5) / 64.0), buckets[b]]
            for b in sorted(buckets)
        ]
    else:
        hist = [[float(lo), n]]
    return {
        "min": float(lo),
        "max": float(hi),
        "mean": float(mean),
        "count": n,
        "histogram_buckets": hist[:MAX_SAMPLE_BUCKETS],
    }


def read_pixel_sample(path: str, x: int, y: int, radius: int = 2) -> dict[str, Any]:
    """Read a small ROI (stats) around (x, y) from the file's primary image.

    Implemented as a best-effort bounding box over tiles: returns the raw
    samples inside the box, or, if that would be too large, compact statistics.
    For simplicity, we read the tile containing (x, y) and report its local
    statistics plus the requested coordinate.
    """
    with _ptiff.open(path) as doc:
        img = doc.image(0)
        md = _metadata_for(img, tile_byte_size(img), str(path))
        if not (0 <= x < md.width) or not (0 <= y < md.height):
            raise PtiffError(f"pixel ({x},{y}) outside {md.width}x{md.height}")
        # Determine the tile containing (x, y). Tile width/height come straight
        # off the Image; the grid position follows from x//tile_width.
        twidth = int(img.tile_width) or _ceil_div(md.width, 1)
        theight = int(img.tile_height) or _ceil_div(md.height, 1)
        col = x // twidth
        row = y // theight
        arr = img.read_tile(col, row)
        analysis = _analyze_samples(arr.tobytes(), md)
        analysis["x"] = x
        analysis["y"] = y
        analysis["tile"] = {"column": col, "row": row}
        analysis["note"] = "stats reported for the tile containing the requested pixel"
        return analysis


def _ceil_div(a: int, b: int) -> int:
    return -(-a // b) if b else a


# ---------------------------------------------------------------------------
# Write side
# ---------------------------------------------------------------------------

# Map MCP param -> PyO3 pixel / compression codes.
_PIXEL_CODES = {v: k for k, v in _PIXEL_NAMES.items() if v != "float64"}
_COMPRESSION_CODES = {
    "none": _ptiff.PTIFF_COMPRESSION_NONE,
    "lzw": _ptiff.PTIFF_COMPRESSION_LZW,
    "deflate": _ptiff.PTIFF_COMPRESSION_DEFLATE,
    "jpeg": _ptiff.PTIFF_COMPRESSION_JPEG,
}

# Keep a tiny in-process registry of open sinks so write_tile/close_image can
# address a sink created via create_image without holding handles over MCP.
# Each registry entry is the PyO3 Sink plus its info.
_open_sinks: dict[str, Any] = {}  # sink_key -> ptiff_pyo3.Sink
_sink_info: dict[str, dict[str, Any]] = {}


def create_image(
    path: str,
    width: int,
    height: int,
    pixel_type: str = "uint8",
    channel_count: int = 1,
    tile_width: int = 64,
    tile_height: int = 64,
    compression: str | None = None,
    camera: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Create a new tiled TIFF/BigTIFF file; returns the open sink info.

    Uses ``ptiff_pyo3.create_image`` (which persists a Camera when given) and
    keeps the write-side Sink in the in-process registry.
    """
    if pixel_type not in _PIXEL_CODES:
        raise PtiffError(
            f"unsupported pixel type '{pixel_type}' (must be uint8/uint16/uint32/float32; "
            "float64 is not a supported TIFF sample type)"
        )
    if channel_count not in (1, 3):
        raise PtiffError("channel_count must be 1 or 3 for TIFF tile I/O")
    if not (tile_width > 0 and tile_height > 0):
        raise PtiffError("tile_width/tile_height must be positive")
    if tile_width % 16 != 0 or tile_height % 16 != 0:
        raise PtiffError(
            f"tile_width/tile_height must be nonzero multiples of 16 for TIFF "
            f"tile writes (got {tile_width}x{tile_height})"
        )
    comp_code = _COMPRESSION_CODES.get(compression if compression else "none")
    if comp_code is None:
        raise PtiffError(f"unsupported compression '{compression}'")

    try:
        sink = _ptiff.create_image(
            str(path),
            int(width),
            int(height),
            pixel_type=_PIXEL_CODES[pixel_type],
            channel_count=int(channel_count),
            tile_width=int(tile_width),
            tile_height=int(tile_height),
            compression=comp_code,
            camera=_camera_to_struct(camera) if camera else None,
        )
    except Exception as exc:
        raise PtiffError(f"create {path}: {exc}") from exc

    cols = int(sink.tile_columns) or 0
    rows = int(sink.tile_rows) or 0
    bs = int(sink.tile_byte_size) or 0
    sink_key = f"sink:{path}"
    _open_sinks[sink_key] = sink
    _sink_info[sink_key] = {
        "path": path,
        "pixel_type": pixel_type,
        "channel_count": channel_count,
        "tile_width": tile_width,
        "tile_height": tile_height,
        "tile_byte_size": bs,
    }
    return {
        "path": path,
        "width": width,
        "height": height,
        "pixel_type": pixel_type,
        "channel_count": channel_count,
        "tile_width": tile_width,
        "tile_height": tile_height,
        "compression": compression or "none",
        "tile_columns": cols,
        "tile_rows": rows,
        "tile_byte_size": bs,
        "sink_key": sink_key,
        "note": "file header + IFD written; call write_tile for each tile, then close_image",
    }


def _camera_to_struct(camera: dict[str, Any]):
    """Build a ptiff_pyo3.Camera from a flat MCP dict, if possible."""
    try:
        return _ptiff.Camera(
            focal_length_x=float(camera.get("focal_length_x", 0.0)),
            focal_length_y=float(camera.get("focal_length_y", 0.0)),
            principal_x=float(camera.get("principal_x", 0.0)),
            principal_y=float(camera.get("principal_y", 0.0)),
            rotation_w=float(camera.get("rotation_w", 1.0)),
            rotation_x=float(camera.get("rotation_x", 0.0)),
            rotation_y=float(camera.get("rotation_y", 0.0)),
            rotation_z=float(camera.get("rotation_z", 0.0)),
            position_x=float(camera.get("position_x", 0.0)),
            position_y=float(camera.get("position_y", 0.0)),
            position_z=float(camera.get("position_z", 0.0)),
            timestamp=str(camera.get("timestamp", "")),
        )
    except Exception:  # noqa: BLE001 -- a bad camera dict should not fail the create
        return None


def _sink_for(key: str) -> Any:
    sink = _open_sinks.get(key)
    if sink is None:
        raise PtiffError(f"no open sink for '{key}' (call create_image first)")
    return sink


def write_tile(sink_key: str, column: int, row: int, data: bytes) -> dict[str, Any]:
    """Write one tile of raw sample data to an open sink."""
    sink = _sink_for(sink_key)
    info = _sink_info.get(sink_key, {})
    cols = int(info.get("tile_columns", 0))
    rows = int(info.get("tile_rows", 0))
    if cols == 0 or rows == 0:
        cols = int(sink.tile_columns) or 0
        rows = int(sink.tile_rows) or 0
    if not (0 <= column < cols) or not (0 <= row < rows):
        raise PtiffError(
            f"tile ({column},{row}) out of range 0..{cols - 1}, 0..{rows - 1}"
        )
    bs = int(sink.tile_byte_size)
    if len(data) != bs:
        raise PtiffError(f"tile data length {len(data)} must equal tile_byte_size {bs}")
    try:
        sink.write_tile(column, row, bytes(data))
    except Exception as exc:
        raise PtiffError(f"write tile ({column},{row}): {exc}") from exc
    return {"sink": sink_key, "column": column, "row": row, "bytes_written": len(data)}


def close_image(sink_key: str) -> dict[str, Any]:
    """Flush and close an open sink (finalizes the file)."""
    sink = _sink_for(sink_key)
    try:
        sink.close()
    except Exception:  # noqa: BLE001, S110 -- Sink.close is idempotent-tolerant
        pass
    _open_sinks.pop(sink_key, None)
    _sink_info.pop(sink_key, None)
    return {"closed": sink_key, "ok": True}


def sink_info(sink_key: str) -> dict[str, Any]:
    """Return layout info for an open sink (pixel type, channels, tile size)."""
    info = _sink_info.get(sink_key)
    if info is None:
        _sink_for(sink_key)  # raise a readable error if truly absent
        raise PtiffError(f"no layout info for '{sink_key}'")
    return dict(info)


def write_image_file(
    path: str,
    width: int,
    height: int,
    pixel_type: str = "uint8",
    channel_count: int = 1,
    tile_width: int = 64,
    tile_height: int = 64,
    compression: str | None = None,
    *,
    fill_value: float | None = None,
    camera: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Create a new tiled image and (optionally) fill every tile with a value.

    Stateless convenience wrapper: closes the sink itself, so the returned
    file is fully readable. If ``fill_value`` is None, tiles are zero-filled.
    ``camera`` (optional) persists a structured camera calibration.
    """
    info = create_image(
        path=path,
        width=width,
        height=height,
        pixel_type=pixel_type,
        channel_count=channel_count,
        tile_width=tile_width,
        tile_height=tile_height,
        compression=compression,
        camera=camera,
    )
    sink_key = info["sink_key"]
    bs = info["tile_byte_size"]
    cols = info["tile_columns"]
    rows = info["tile_rows"]
    try:
        if fill_value is None:
            payload = b"\x00" * bs
        else:
            payload = _pack_fill(pixel_type, channel_count, fill_value, bs)
        for c in range(cols):
            for r in range(rows):
                write_tile(sink_key, c, r, payload)
        return info
    finally:
        close_image(sink_key)


def _pack_fill(pixel_type: str, channel_count: int, value: float, size: int) -> bytes:
    """Pack a constant sample value into a raw tile buffer of the given size."""
    import struct

    fmt = {"uint8": "B", "uint16": "H", "uint32": "I", "float32": "f"}.get(pixel_type)
    if fmt is None:
        raise PtiffError(f"unsupported pixel type '{pixel_type}' for fill")
    ch = max(1, channel_count)
    elem = struct.pack(fmt, int(value))
    if ch == 1:
        return elem * (size // len(elem))
    # multi-channel: repeat channel-by-channel
    unit = elem * ch
    block = unit * (size // len(unit))
    # pad any remainder
    return block + b"\x00" * (size - len(block))
