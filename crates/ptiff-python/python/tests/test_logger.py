"""Logger surface (feature parity with SWIG ptiff.logger)."""

import ptiff_pyo3 as ptiff


def test_logger_roundtrip():
    original = ptiff.Logger.level()
    try:
        ptiff.Logger.set_level(ptiff.PTIFF_LOG_ERROR)
        assert ptiff.Logger.level() == ptiff.PTIFF_LOG_ERROR

        # Emitting at/below the threshold must be safe.
        ptiff.Logger.log(ptiff.PTIFF_LOG_TRACE, "filtered trace")
        ptiff.Logger.log(ptiff.PTIFF_LOG_INFO, "filtered info")
        ptiff.Logger.log(ptiff.PTIFF_LOG_ERROR, "emitted error")

        ptiff.Logger.set_level(ptiff.PTIFF_LOG_DEBUG)
        assert ptiff.Logger.level() == ptiff.PTIFF_LOG_DEBUG
        ptiff.Logger.log(ptiff.PTIFF_LOG_WARN, "a warning")
    finally:
        ptiff.Logger.set_level(original)


def test_logger_level_constants_on_class():
    assert ptiff.Logger.TRACE == ptiff.PTIFF_LOG_TRACE
    assert ptiff.Logger.DEBUG == ptiff.PTIFF_LOG_DEBUG
    assert ptiff.Logger.INFO == ptiff.PTIFF_LOG_INFO
    assert ptiff.Logger.WARN == ptiff.PTIFF_LOG_WARN
    assert ptiff.Logger.ERROR == ptiff.PTIFF_LOG_ERROR
    assert ptiff.Logger.CRITICAL == ptiff.PTIFF_LOG_CRITICAL
    assert ptiff.Logger.OFF == ptiff.PTIFF_LOG_OFF
