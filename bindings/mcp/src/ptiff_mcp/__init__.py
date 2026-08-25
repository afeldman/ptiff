"""ptiff_mcp — MCP server for PTIFF (Planetary TIFF) 1.0.

Thin stdio application layer over the PyO3 ``ptiff_pyo3`` binding (which reads
the PTIFF Rust core directly). Promoted to 1.0.0 (official) as of 2026-08-25:
10 tools, 9/9 tests green (in-process dispatch + real stdio MCP round-trip).
"""

from . import runtime  # noqa: F401
from .server import main  # noqa: F401
