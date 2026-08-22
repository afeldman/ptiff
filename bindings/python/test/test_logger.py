"""Logger surface over the raw SWIG-generated Python binding.

Ports bindings/python/test/test_ptiff.py's test_logger_roundtrip. There is no
Logger class here (that is hand-written-binding sugar) -- the C ABI's actual
promise is the free functions ptiff_logger_set_level/ptiff_logger_level and
that emitting a log line below/at the threshold never crashes.
"""

import unittest

import ptiff


class TestLogger(unittest.TestCase):
    def test_logger_roundtrip(self) -> None:
        original = ptiff.ptiff_logger_level()
        self.addCleanup(ptiff.ptiff_logger_set_level, original)

        ptiff.ptiff_logger_set_level(ptiff.PTIFF_LOG_ERROR)
        self.assertEqual(ptiff.ptiff_logger_level(), ptiff.PTIFF_LOG_ERROR)

        # Emitting a log at a level below the current threshold must be safe.
        ptiff.ptiff_logger_log(ptiff.PTIFF_LOG_TRACE, "this trace line is filtered out")
        ptiff.ptiff_logger_log(ptiff.PTIFF_LOG_INFO, "this info line is filtered out")
        ptiff.ptiff_logger_log(ptiff.PTIFF_LOG_ERROR, "this error line is emitted")

        ptiff.ptiff_logger_set_level(ptiff.PTIFF_LOG_DEBUG)
        self.assertEqual(ptiff.ptiff_logger_level(), ptiff.PTIFF_LOG_DEBUG)
        ptiff.ptiff_logger_log(ptiff.PTIFF_LOG_DEBUG, "backend debugging")
        ptiff.ptiff_logger_log(ptiff.PTIFF_LOG_WARN, "a warning")


if __name__ == "__main__":
    unittest.main()
