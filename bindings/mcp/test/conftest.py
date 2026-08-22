"""pytest roots configured here so libptiff_c is discoverable before the
ptiff binding is imported by the test/server modules.
"""

import os

_HERE = os.path.dirname(os.path.abspath(__file__))
_MCP = os.path.dirname(_HERE)
_REPO = os.path.dirname(os.path.dirname(os.path.dirname(_HERE)))

_SHARED = os.path.join(_REPO, "build-shared", "build", "Release")
_PTIFF_C = os.path.join(_SHARED, "bindings", "c")
_PTIFF_LIB = os.path.join(_SHARED, "libptiff")

os.environ.setdefault("PTIFF_C_LIB_DIR", _PTIFF_C)
os.environ.setdefault("PTIFF_LIB_DIR", _PTIFF_LIB)

# Also expose the SWIG ptiff binding on sys.path for the whole session.
import sys  # noqa: E402

_ADD = [
    os.path.join(_MCP, "src"),
    os.path.join(_REPO, "bindings", "python", "src"),
]
for p in _ADD:
    if p not in sys.path:
        sys.path.insert(0, p)
