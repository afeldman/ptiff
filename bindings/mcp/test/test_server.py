"""Tests for the PTIFF MCP server.

Two flavours:

* In-process unit tests of the runtime dispatch (no MCP transport), using a
    reference TIFF generated on the fly with ``ptiff_pyo3`` (see conftest).
* A real stdio MCP round-trip that boots ``python -m ptiff_mcp.server`` as a
    child process, initializes it, lists tools and executes a metadata read.

Run from bindings/mcp (with ``ptiff_pyo3`` installed into the venv):

    .venv/bin/python -m pytest -q
"""

from __future__ import annotations

import os
from pathlib import Path

import anyio
import pytest
from ptiff_mcp import server

# Environment for child-process stdio round-trip: expose the MCP source and
# rely on the installed ``ptiff_pyo3`` in the active venv.
_ENV = dict(os.environ)
_REPO = Path(__file__).resolve().parents[3]
_MCP = Path(__file__).resolve().parents[1]
_ENV["PYTHONPATH"] = str(_MCP / "src")
_PY = str(_MCP / ".venv" / "bin" / "python")


# ---------------------------------------------------------------------------
# In-process dispatch tests
# ---------------------------------------------------------------------------


def test_get_version() -> None:
    v = server._dispatch("get_version", None)
    assert v["runtime"].count(".") == 2
    assert v["compile_time"].count(".") == 2


def test_list_backends() -> None:
    b = server._dispatch("list_backends", None)
    assert isinstance(b, list)
    assert "tiff" in b
    assert len(b) >= 1


def test_read_metadata(sample_tiff: str) -> None:
    md = server._dispatch("read_metadata", {"path": sample_tiff})
    assert md["width"] == 32
    assert md["height"] == 32
    assert md["pixel_type"] == "uint8"
    assert md["tile_columns"] == 2
    assert md["tile_rows"] == 2


def test_read_tile_stats(sample_tiff: str) -> None:
    t = server._dispatch("read_tile", {"path": sample_tiff, "column": 0, "row": 0})
    assert t["channel_count"] == 1
    assert t["sample_count"] == 256
    assert t["channels"][0]["count"] == 256
    assert t["channels"][0]["min"] <= t["channels"][0]["max"]


def test_write_image_file_roundtrip(tmp_path: Path) -> None:
    out = tmp_path / "fill.tif"
    info = server._dispatch(
        "write_image_file",
        {
            "path": str(out),
            "width": 16,
            "height": 16,
            "pixel_type": "uint16",
            "channel_count": 1,
            "tile_width": 16,
            "tile_height": 16,
            "fill_value": 258,
        },
    )
    assert info["tile_columns"] == 1
    assert info["tile_rows"] == 1
    md = server._dispatch("read_metadata", {"path": str(out)})
    assert md["width"] == 16
    t = server._dispatch("read_tile", {"path": str(out), "column": 0, "row": 0})
    assert t["channels"][0]["min"] == 258.0
    assert t["channels"][0]["max"] == 258.0


def test_write_image_file_with_camera_roundtrip(tmp_path: Path) -> None:
    """Camera persisted on write is readable back (structured + fields)."""
    out = tmp_path / "cam.tif"
    info = server._dispatch(
        "write_image_file",
        {
            "path": str(out),
            "width": 16,
            "height": 16,
            "pixel_type": "uint8",
            "channel_count": 1,
            "tile_width": 16,
            "tile_height": 16,
            "fill_value": 7,
            "camera": {
                "focal_length_x": 700.0,
                "focal_length_y": 715.0,
                "principal_x": 8.0,
                "principal_y": 8.0,
                "position_x": 1.0,
                "position_y": 2.0,
                "position_z": 3.0,
                "timestamp": "2026-08-21T12:34:56.000Z",
            },
        },
    )
    assert info["tile_columns"] == 1 and info["tile_rows"] == 1

    md = server._dispatch("read_metadata", {"path": str(out)})
    cam = md.get("camera")
    assert cam is not None
    assert cam["has_intrinsics"] is True
    assert cam["focal_length_x"] == 700.0
    assert cam["timestamp"] == "2026-08-21T12:34:56.000Z"
    # The camera is exposed as a structured ``camera`` dict (not as a flattened
    # ``fields['ptiff.camera.model']`` extension field) in the PyO3 runtime.
    assert cam.get("has_extrinsics") is True


def test_read_metadata_camera_and_fields() -> None:
    """Against the interop fixture: extension fields + structured camera.

    Skip when the fixture is not present — the MCP tests otherwise only use
    the generated reference TIFF and need no foreign files.
    """
    fixture = str(_REPO / "scripts" / "samples" / "ptiff_interop_fixture.tif")
    if not os.path.exists(fixture):
        pytest.skip("interop fixture not present")
    md = server._dispatch("read_metadata", {"path": fixture})
    assert md["width"] == 128 and md["height"] == 128
    # Generic extension fields (SPICE/scientific layers/provenance) surface
    # as flattened ``ptiff.<domain>.<key>`` fields in the PyO3 runtime.
    fields = md.get("fields", {})
    assert fields.get("ptiff.spice.frame") == "IAU_MOON"
    # The camera is exposed as a structured ``camera`` dict, not a flattened
    # field (the PyO3 binding separates the camera domain from generic fields).
    cam = md.get("camera")
    assert cam is not None and cam["has_intrinsics"] is True


def _call_tool(name: str, args: dict | None):
    return server._handle(name, args)


def test_error_surfacing(tmp_path: Path) -> None:
    from mcp.types import CallToolResult

    res = _call_tool("read_metadata", {"path": str(tmp_path / "nope.tif")})
    assert isinstance(res, CallToolResult)
    assert res.is_error is True
    assert "error" in res.content[0].text


# ---------------------------------------------------------------------------
# Real stdio MCP round-trip
# ---------------------------------------------------------------------------


def test_stdio_roundtrip() -> None:
    from mcp import ClientSession
    from mcp.client.stdio import StdioServerParameters, stdio_client

    params = StdioServerParameters(
        command=_PY if os.path.exists(_PY) else str(_ENV.get("PYTHON") or "python"),
        args=["-m", "ptiff_mcp.server"],
        env=_ENV,
    )
    _ENV["PYTHON"] = params.command  # not needed but harmless

    async def run() -> None:
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()
                tools = await session.list_tools()
                names = [t.name for t in tools.tools]
                assert "get_version" in names
                assert "read_metadata" in names
                assert "write_image_file" in names
                res = await session.call_tool("get_version", {})
                assert res.is_error is False
                text = res.content[0].text
                assert "0." in text  # e.g. "1.0.0"

    anyio.run(run, backend="trio")
