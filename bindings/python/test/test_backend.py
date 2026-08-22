"""Backend registry surface over the raw SWIG-generated Python binding.

Ports bindings/python/test/test_ptiff.py's test_backends. The C ABI returns a
single comma-space-joined, caller-owned C string (documented as released via
ptiff_free_string). SWIG's default `char*` return typemap for Python already
copies the C buffer into a Python str (same as the Go binding's swigCopyString
-- see bindings/swig/go/ptiff.go's Ptiff_backend_names), so what test code
gets back is a Python-owned copy, not the original pointer: calling
ptiff_free_string on it passes SWIG's own string-conversion buffer, not the
pointer ptiff_backend_names() malloc'd, and aborts. The original buffer is
leaked by the generated binding, same as the Go port -- not fixed here, since
fixing it needs a %newobject/custom typemap in ptiff.i, not a test change.
"""

import unittest

import ptiff


class TestBackend(unittest.TestCase):
    def test_backend_names(self) -> None:
        joined = ptiff.ptiff_backend_names()
        # May legitimately be empty when linking statically without
        # whole-archive (backend registration TUs get dead-stripped).
        if not joined:
            return
        names = joined.split(", ")
        self.assertTrue(all(names), f"empty name in {joined!r}")


if __name__ == "__main__":
    unittest.main()
