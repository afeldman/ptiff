"""Tests for the PTIFF MCP server.

Two flavours:

* In-process unit tests of the runtime dispatch (no MCP transport), using the
    fixture TIFF written alongside the Python bindings.
* A real stdio MCP round-trip that boots ``python -m ptiff_mcp.server`` as a
    child process, initializes it, lists tools and executes a metadata read.

Run from bindings/mcp after building libptiff_c (see DESIGN.md; the Rust crate
crates/ptiff-c builds it via `cargo build -p ptiff-c --release`, and env vars
below point at target/release):

    PTIFF_C_LIB_DIR=.../target/release PTIFF_LIB_DIR=.../target/release pytest -q
"""

from __future__ import annotations

import os
from pathlib import Path

import anyio
import pytest
from ptiff_mcp import server

# Environment: point at the Rust-built libptiff_c (cargo build -p ptiff-c).
_ENV = dict(os.environ)
_REPO = Path(__file__).resolve().parents[3]
_TARGET = _REPO / "target" / "release"
_ENV.setdefault("PTIFF_C_LIB_DIR", str(_TARGET))
_ENV.setdefault("PTIFF_LIB_DIR", str(_TARGET))
_ENV["PYTHONPATH"] = (
    str(Path(__file__).resolve().parents[1] / "src")
    + os.pathsep
    + str(_REPO / "bindings" / "python" / "src")
)
_PY = str(Path(__file__).resolve().parents[1] / ".venv" / "bin" / "python")

# The example TIFF written by the Python bindings' roundtrip test.
SAMPLE = str(_REPO / "bindings" / "python" / "test" / "roundtrip_python.tif")


def _require_env() -> None:
    if not os.path.exists(_ENV["PTIFF_C_LIB_DIR"]):
        pytest.skip(
            "libptiff_c not built (expected at target/release; "
            "run `cargo build -p ptiff-c --release`). See DESIGN.md"
        )


# ---------------------------------------------------------------------------
# In-process dispatch tests
# ---------------------------------------------------------------------------


def test_get_version() -> None:
    _require_env()
    v = server._dispatch("get_version", None)
    assert v["runtime"].count(".") == 2
    assert v["compile_time"].count(".") == 2


def test_list_backends() -> None:
    _require_env()
    b = server._dispatch("list_backends", None)
    assert isinstance(b, list)
    assert "tiff" in b
    assert len(b) >= 1


def test_read_metadata() -> None:
    _require_env()
    md = server._dispatch("read_metadata", {"path": SAMPLE})
    assert md["width"] == 32
    assert md["height"] == 32
    assert md["pixel_type"] == "uint8"
    assert md["tile_columns"] == 2
    assert md["tile_rows"] == 2


def test_read_tile_stats() -> None:
    _require_env()
    t = server._dispatch("read_tile", {"path": SAMPLE, "column": 0, "row": 0})
    assert t["channel_count"] == 1
    assert t["sample_count"] == 256
    assert t["channels"][0]["count"] == 256
    assert t["channels"][0]["min"] <= t["channels"][0]["max"]


def test_write_image_file_roundtrip(tmp_path: Path) -> None:
    _require_env()
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
    _require_env()
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
    fields = md.get("fields", {})
    assert fields.get("ptiff.camera.model") == "pinhole"


def test_read_metadata_camera_and_fields() -> None:
    """Against the interop fixture: extension fields + structured camera."""
    _require_env()
    fixture = str(_REPO / "scripts" / "samples" / "ptiff_interop_fixture.tif")
    if not os.path.exists(fixture):
        pytest.skip("interop fixture not present")
    md = server._dispatch("read_metadata", {"path": fixture})
    assert md["width"] == 128 and md["height"] == 128
    fields = md.get("fields", {})
    assert fields.get("ptiff.camera.model") == "pinhole"
    assert fields.get("ptiff.spice.frame") == "IAU_MOON"
    cam = md.get("camera")
    assert cam is not None and cam["has_intrinsics"] is True
    assert cam["focal_length_x"] == 700.0


def _call_tool(name: str, args: dict | None):
    return server._handle(name, args)


def test_error_surfacing(tmp_path: Path) -> None:
    _require_env()
    from mcp.types import CallToolResult

    res = _call_tool("read_metadata", {"path": str(tmp_path / "nope.tif")})
    assert isinstance(res, CallToolResult)
    assert res.is_error is True
    assert "error" in res.content[0].text


# ---------------------------------------------------------------------------
# Real stdio MCP round-trip
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    not os.path.exists(_ENV.get("PTIFF_C_LIB_DIR", "")), reason="libptiff_c not built"
)
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
