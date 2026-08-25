"""PTIFF MCP server.

Exposes libptiff (Planetary TIFF) to an LLM over the Model Context Protocol,
running over stdio. Thin application layer: all real work happens in
``runtime`` against the PyO3 ``ptiff_pyo3`` binding over the PTIFF Rust core.

Run (from bindings/mcp; ptiff_pyo3 is built via `maturin` in
crates/ptiff-python and installed into this venv):

    .venv/bin/python -m ptiff_mcp.server

See DESIGN.md for tool semantics and the deliberate context-size limits.
"""

from __future__ import annotations

import sys
from typing import Any

import anyio
from mcp.server.lowlevel.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import CallToolRequestParams, CallToolResult, TextContent, Tool

from . import runtime

SERVER_NAME = "ptiff"
SERVER_TITLE = "PTIFF — Planetary TIFF"
SERVER_DESC = (
    "Read and write PTIFF / libptiff planetary image documents: metadata, "
    "tile pixel statistics, and small tiled writes, over the MCP protocol."
)

# ---------------------------------------------------------------------------
# Tool registry
# ---------------------------------------------------------------------------

# Each tool: name -> (title, description, input JSON schema)
_TOOLS: dict[str, tuple[str, str, dict[str, Any]]] = {
    "get_version": (
        "Get libptiff version",
        "Return the runtime and compile-time version of the linked libptiff.",
        {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    ),
    "list_backends": (
        "List registered backends",
        "Return the backend names registered in the linked libptiff "
        "(e.g. tiff, pds4, isis, zarr, openexr, memory).",
        {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
    ),
    "read_metadata": (
        "Read image metadata",
        "Read the primary image metadata of a PTIFF/TIFF file: dimensions, "
        "pixel type, channel count, tile layout, compression.",
        {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the TIFF/BigTIFF/PTIFF/[...] file.",
                }
            },
            "required": ["path"],
            "additionalProperties": False,
        },
    ),
    "read_scene": (
        "Read scene metadata",
        "Alias for read_metadata exposing the PTIFF 'scene' view of the "
        "primary image. Returns the same fields.",
        {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file."}
            },
            "required": ["path"],
            "additionalProperties": False,
        },
    ),
    "read_tile": (
        "Read tile statistics",
        "Read one tile/strip of the primary image and return compact pixel "
        "statistics (min/max/mean, log-binned histogram) per channel. Never "
        "returns raw raster buffers.",
        {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file."},
                "column": {
                    "type": "integer",
                    "description": "Tile column (0-based).",
                },
                "row": {"type": "integer", "description": "Tile row (0-based)."},
            },
            "required": ["path", "column", "row"],
            "additionalProperties": False,
        },
    ),
    "read_pixel_sample": (
        "Read pixel-statistics around a point",
        "Report compact statistics of the tile containing pixel (x, y). Use "
        "x/y in image pixel coordinates.",
        {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file."},
                "x": {
                    "type": "integer",
                    "description": "Image x (column) coordinate.",
                },
                "y": {"type": "integer", "description": "Image y (row) coordinate."},
            },
            "required": ["path", "x", "y"],
            "additionalProperties": False,
        },
    ),
    "write_image_file": (
        "Create a new tiled image",
        "Create a new tiled TIFF/BigTIFF file, optionally filling every tile "
        "with a constant value. Stateless: the file is complete and readable "
        "on return.",
        {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Output path."},
                "width": {"type": "integer", "description": "Image width (px)."},
                "height": {"type": "integer", "description": "Image height (px)."},
                "pixel_type": {
                    "type": "string",
                    "enum": ["uint8", "uint16", "uint32", "float32"],
                    "default": "uint8",
                },
                "channel_count": {
                    "type": "integer",
                    "enum": [1, 3],
                    "default": 1,
                },
                "tile_width": {
                    "type": "integer",
                    "default": 64,
                    "description": "Tile width (px); nonzero multiple of 16.",
                },
                "tile_height": {
                    "type": "integer",
                    "default": 64,
                    "description": "Tile height (px); nonzero multiple of 16.",
                },
                "compression": {
                    "type": "string",
                    "enum": ["none", "lzw", "deflate", "jpeg"],
                    "default": "none",
                },
                "fill_value": {
                    "type": "number",
                    "description": "Constant value to fill (0 fills with zeros).",
                },
                "camera": {
                    "type": "object",
                    "description": "Optional structured pinhole camera to persist "
                    "as metadata (K = fx,cx; fy,cy; position t = world origin). "
                    "Keys: focal_length_x/y, principal_x/y, rotation_w/x/y/z, "
                    "position_x/y/z, timestamp.",
                    "properties": {
                        "focal_length_x": {"type": "number"},
                        "focal_length_y": {"type": "number"},
                        "principal_x": {"type": "number"},
                        "principal_y": {"type": "number"},
                        "rotation_w": {"type": "number"},
                        "rotation_x": {"type": "number"},
                        "rotation_y": {"type": "number"},
                        "rotation_z": {"type": "number"},
                        "position_x": {"type": "number"},
                        "position_y": {"type": "number"},
                        "position_z": {"type": "number"},
                        "timestamp": {"type": "string"},
                    },
                    "additionalProperties": True,
                },
            },
            "required": ["path", "width", "height"],
            "additionalProperties": False,
        },
    ),
    "create_image": (
        "Create a tiled image (low-level)",
        "Open a new tiled image for sequential tile writes. Returns a sink_key "
        "that subsequent write_tile / close_image calls must pass. Use "
        "write_image_file instead unless you need per-tile control. "
        "tile_width/tile_height must be nonzero multiples of 16.",
        {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Output path."},
                "width": {"type": "integer", "description": "Image width (px)."},
                "height": {"type": "integer", "description": "Image height (px)."},
                "pixel_type": {
                    "type": "string",
                    "enum": ["uint8", "uint16", "uint32", "float32"],
                    "default": "uint8",
                },
                "channel_count": {"type": "integer", "enum": [1, 3], "default": 1},
                "tile_width": {
                    "type": "integer",
                    "default": 64,
                    "description": "Tile width (px); nonzero multiple of 16.",
                },
                "tile_height": {
                    "type": "integer",
                    "default": 64,
                    "description": "Tile height (px); nonzero multiple of 16.",
                },
                "compression": {"type": "string", "default": "none"},
                "camera": {
                    "type": "object",
                    "description": "Optional structured pinhole camera to persist "
                    "as metadata (K = fx,cx; fy,cy; position t = world origin). "
                    "Keys: focal_length_x/y, principal_x/y, rotation_w/x/y/z, "
                    "position_x/y/z, timestamp.",
                    "properties": {
                        "focal_length_x": {"type": "number"},
                        "focal_length_y": {"type": "number"},
                        "principal_x": {"type": "number"},
                        "principal_y": {"type": "number"},
                        "rotation_w": {"type": "number"},
                        "rotation_x": {"type": "number"},
                        "rotation_y": {"type": "number"},
                        "rotation_z": {"type": "number"},
                        "position_x": {"type": "number"},
                        "position_y": {"type": "number"},
                        "position_z": {"type": "number"},
                        "timestamp": {"type": "string"},
                    },
                    "additionalProperties": True,
                },
            },
            "required": ["path", "width", "height"],
            "additionalProperties": False,
        },
    ),
    "write_tile": (
        "Write a single tile",
        "Write one tile's raw sample data (hex string or a constant fill) to "
        "an open sink created with create_image. Call close_image afterwards "
        "to finalize.",
        {
            "type": "object",
            "properties": {
                "sink_key": {"type": "string", "description": "From create_image."},
                "column": {"type": "integer", "description": "Tile column (0-based)."},
                "row": {"type": "integer", "description": "Tile row (0-based)."},
                "data_hex": {
                    "type": "string",
                    "description": "Raw tile bytes as a hex string (length must "
                    "equal tile_byte_size).",
                },
                "fill": {
                    "type": "number",
                    "description": "If set, writes a constant-filled tile.",
                },
            },
            "anyOf": [
                {"required": ["sink_key", "column", "row", "data_hex"]},
                {"required": ["sink_key", "column", "row", "fill"]},
            ],
            "additionalProperties": False,
        },
    ),
    "close_image": (
        "Close an open image for writing",
        "Flush and close a sink previously opened with create_image, making "
        "the written file complete and readable.",
        {
            "type": "object",
            "properties": {
                "sink_key": {"type": "string", "description": "From create_image."}
            },
            "required": ["sink_key"],
            "additionalProperties": False,
        },
    ),
}


# Store a persistent map name->Tool for list_tools.
def _build_tools() -> list[Tool]:
    tools: list[Tool] = []
    for name, (_title, desc, schema) in _TOOLS.items():
        tools.append(
            Tool(name=name, title=_title, description=desc, input_schema=schema)
        )
    return tools


_TOOL_LIST = _build_tools()


# ---------------------------------------------------------------------------
# Argument parsing + dispatch
# ---------------------------------------------------------------------------


def _get(args: dict[str, Any] | None, key: str, default: Any = None) -> Any:
    if not args:
        return default
    return args.get(key, default)


def _dispatch(name: str, args: dict[str, Any] | None) -> Any:
    """Dispatch a tool call to runtime, returning JSON-serializable data."""
    if name == "get_version":
        return runtime.get_version()
    if name == "list_backends":
        return runtime.list_backends()
    if name == "read_metadata":
        path = _get(args, "path")
        if not path:
            raise runtime.PtiffError("missing 'path'")
        return runtime.metadata_to_dict(runtime.read_metadata(path))
    if name == "read_scene":
        path = _get(args, "path")
        if not path:
            raise runtime.PtiffError("missing 'path'")
        return {"scene": runtime.metadata_to_dict(runtime.read_metadata(path))}
    if name == "read_tile":
        return runtime.read_tile(
            _get(args, "path"),
            int(_get(args, "column")),
            int(_get(args, "row")),
        )
    if name == "read_pixel_sample":
        return runtime.read_pixel_sample(
            _get(args, "path"),
            int(_get(args, "x")),
            int(_get(args, "y")),
        )
    if name == "write_image_file":
        return runtime.write_image_file(
            path=_get(args, "path"),
            width=int(_get(args, "width")),
            height=int(_get(args, "height")),
            pixel_type=_get(args, "pixel_type", "uint8"),
            channel_count=int(_get(args, "channel_count", 1)),
            tile_width=int(_get(args, "tile_width", 64)),
            tile_height=int(_get(args, "tile_height", 64)),
            compression=_get(args, "compression", "none"),
            fill_value=_get(args, "fill_value", None),
            camera=_get(args, "camera", None),
        )
    if name == "create_image":
        return runtime.create_image(
            path=_get(args, "path"),
            width=int(_get(args, "width")),
            height=int(_get(args, "height")),
            pixel_type=_get(args, "pixel_type", "uint8"),
            channel_count=int(_get(args, "channel_count", 1)),
            tile_width=int(_get(args, "tile_width", 64)),
            tile_height=int(_get(args, "tile_height", 64)),
            compression=_get(args, "compression", "none"),
            camera=_get(args, "camera", None),
        )
    if name == "write_tile":
        return runtime.write_tile(
            _get(args, "sink_key"),
            int(_get(args, "column")),
            int(_get(args, "row")),
            _tile_payload(args),
        )
    if name == "close_image":
        return runtime.close_image(_get(args, "sink_key"))
    raise runtime.PtiffError(f"unknown tool '{name}'")


def _tile_payload(args: dict[str, Any] | None) -> bytes:
    fill = _get(args, "fill")
    data_hex = _get(args, "data_hex")
    if fill is not None and data_hex:
        raise runtime.PtiffError("pass either 'data_hex' or 'fill', not both")
    if data_hex:
        try:
            return bytes.fromhex(str(data_hex))
        except ValueError as exc:
            raise runtime.PtiffError(f"invalid 'data_hex': {exc}") from exc
    if fill is not None:
        # Use zero-length? Instead pack according to the open sink's pixel type.
        # We need the sink's pixel type; recover from the registry info by the
        # sink path (stored in sink_key). Simplest: fill bytes are provided
        # explicitly — call runtime helper that knows pixel layout via sink.
        return _encode_fill_for_sink(_get(args, "sink_key"), fill)
    raise runtime.PtiffError("write_tile requires 'data_hex' or 'fill'")


def _encode_fill_for_sink(sink_key: str, fill: float) -> bytes:
    """Encode a constant fill for the open sink by querying its pixel type."""
    # determine pixel type from the sink's descriptor via runtime helper
    import struct

    info = runtime.sink_info(sink_key)
    pixel = info["pixel_type"]
    channels = info["channel_count"]
    size = info["tile_byte_size"]
    fmt = {"uint8": "B", "uint16": "H", "uint32": "I", "float32": "f"}.get(pixel)
    if fmt is None:
        raise runtime.PtiffError(f"unsupported pixel type '{pixel}' for fill")
    elem = struct.pack(fmt, int(fill))
    ch = max(1, channels)
    if ch == 1:
        return elem * (size // len(elem))
    unit = elem * ch
    block = unit * (size // len(unit))
    return block + b"\x00" * (size - len(block))


# ---------------------------------------------------------------------------
# MCP handlers
# ---------------------------------------------------------------------------


async def _on_list_tools(ctx, params):
    return _list_tools_result()


def _list_tools_result():
    from mcp.types import ListToolsResult

    return ListToolsResult(tools=_TOOL_LIST)


async def _on_call_tool(ctx, params: CallToolRequestParams) -> CallToolResult:
    return _handle(params.name, params.arguments)


def _handle(name: str, args: dict[str, Any] | None) -> CallToolResult:
    """Synchronous tool dispatch; also used directly by in-process tests."""
    try:
        data = _dispatch(name, args)
        text = _serialize(data)
        return CallToolResult(content=[TextContent(type="text", text=text)])
    except Exception as exc:  # noqa: BLE001 -- surface any failure to the client
        text = f"ptiff mcp error: {exc}"
        return CallToolResult(
            content=[TextContent(type="text", text=text)], is_error=True
        )


def _serialize(data: Any) -> str:
    import json

    try:
        return json.dumps(data, indent=2, default=str)
    except (TypeError, ValueError):
        return str(data)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


async def _main() -> None:
    server = Server(
        SERVER_NAME,
        version="0.0.1",
        title=SERVER_TITLE,
        description=SERVER_DESC,
        on_list_tools=_on_list_tools,
        on_call_tool=_on_call_tool,
    )
    async with stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream, write_stream, server.create_initialization_options()
        )


def main() -> None:
    try:
        anyio.run(_main, backend="trio")
    except KeyboardInterrupt:  # pragma: no cover
        sys.exit(0)


if __name__ == "__main__":
    main()
