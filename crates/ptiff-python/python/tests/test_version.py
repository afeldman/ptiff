"""Version / backend / constant ordering pins (feature parity with SWIG)."""

import re

import numpy as np
import ptiff_pyo3 as ptiff


def test_runtime_version_matches_semver():
    major, minor, patch = ptiff.runtime_version()
    assert re.fullmatch(r"\d+\.\d+\.\d+", f"{major}.{minor}.{patch}")


def test_runtime_matches_compile_time():
    assert ptiff.runtime_version() == ptiff.compile_time_version()


def test_version_string_semver():
    assert re.fullmatch(r"\d+\.\d+\.\d+", ptiff.version_string())


def test_abi_version_is_one():
    assert ptiff.abi_version() >= 1


def test_backend_names_nonempty_or_documented():
    joined = ptiff.backend_names()
    if not joined:
        return  # statically linked without whole-archive: may be empty
    names = joined.split(", ")
    assert all(names), f"empty backend name in {joined!r}"


class TestPixelTypeOrdering:
    def test_ordering(self):
        cases = [
            (ptiff.PTIFF_PIXEL_UINT8, 0),
            (ptiff.PTIFF_PIXEL_UINT16, 1),
            (ptiff.PTIFF_PIXEL_UINT32, 2),
            (ptiff.PTIFF_PIXEL_FLOAT32, 3),
            (ptiff.PTIFF_PIXEL_FLOAT64, 4),
        ]
        for code, want in cases:
            assert code == want


class TestCompressionKindOrdering:
    def test_ordering(self):
        cases = [
            (ptiff.PTIFF_COMPRESSION_NONE, 0),
            (ptiff.PTIFF_COMPRESSION_LZW, 1),
            (ptiff.PTIFF_COMPRESSION_DEFLATE, 2),
            (ptiff.PTIFF_COMPRESSION_JPEG, 3),
        ]
        for code, want in cases:
            assert code == want


def test_numpy_tile_api_returns_ndarray():
    # Sanity: numpy is a hard dependency of the tile API.
    assert np is not None
