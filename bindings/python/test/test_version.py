"""Version surface over the raw SWIG-generated Python binding.

Ports the version coverage of bindings/python/test/test_ptiff.py onto the
SWIG output: both the struct-return (ptiff_runtime_version /
ptiff_compile_time_version) and the out-parameter (*_out) forms of the C ABI.
The *_out forms need the OUTPUT typemap added to typemaps.i (SWIGPYTHON
block, "int *major"/"int *minor"/"int *patch") -- without it, a plain int*
C ABI parameter has no pure-Python counterpart to pass by reference.
"""

import unittest

import ptiff


class TestVersion(unittest.TestCase):
    def test_runtime_version_matches_semver(self) -> None:
        v = ptiff.ptiff_runtime_version()
        s = f"{v.major}.{v.minor}.{v.patch}"
        self.assertRegex(s, r"^\d+\.\d+\.\d+$")

    def test_runtime_matches_compile_time(self) -> None:
        r = ptiff.ptiff_runtime_version()
        c = ptiff.ptiff_compile_time_version()
        self.assertEqual((r.major, r.minor, r.patch), (c.major, c.minor, c.patch))

    def test_version_out_params(self) -> None:
        rMaj, rMin, rPat = ptiff.ptiff_runtime_version_out()
        cMaj, cMin, cPat = ptiff.ptiff_compile_time_version_out()
        self.assertEqual((rMaj, rMin, rPat), (cMaj, cMin, cPat))

        v = ptiff.ptiff_runtime_version()
        self.assertEqual((rMaj, rMin, rPat), (v.major, v.minor, v.patch))


if __name__ == "__main__":
    unittest.main()
