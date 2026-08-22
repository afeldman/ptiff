"""Logging wrapper over the SWIG low-level logger surface.

The C ABI exposes three free functions (`ptiff_logger_set_level`,
`ptiff_logger_level`, `ptiff_logger_log`) and the `PTIFF_LOG_*` level
constants. This module wraps them as a tiny ``Logger`` value object so the
idiomatic object layer stays uniform with `Camera`/`Image`/`Tile`.
"""

from .ptiff import (
    PTIFF_LOG_CRITICAL,
    PTIFF_LOG_DEBUG,
    PTIFF_LOG_ERROR,
    PTIFF_LOG_INFO,
    PTIFF_LOG_OFF,
    PTIFF_LOG_TRACE,
    PTIFF_LOG_WARN,
    ptiff_logger_level,
    ptiff_logger_log,
    ptiff_logger_set_level,
)

TRACE = PTIFF_LOG_TRACE
DEBUG = PTIFF_LOG_DEBUG
INFO = PTIFF_LOG_INFO
WARN = PTIFF_LOG_WARN
ERROR = PTIFF_LOG_ERROR
CRITICAL = PTIFF_LOG_CRITICAL
OFF = PTIFF_LOG_OFF


class Logger:
    """A thin, stateless view over the library's global logger.

    The underlying C logger is a single process-wide instance, so these
    methods read and mutate that one instance regardless of how many
    ``Logger`` objects exist.
    """

    # Level constants exposed on the class for convenience.
    TRACE = TRACE
    DEBUG = DEBUG
    INFO = INFO
    WARN = WARN
    ERROR = ERROR
    CRITICAL = CRITICAL
    OFF = OFF

    @property
    def level(self) -> int:
        return ptiff_logger_level()

    @level.setter
    def level(self, value: int) -> None:
        ptiff_logger_set_level(int(value))

    def set_level(self, level: int) -> None:
        """Set the minimum level that gets emitted (see the PTIFF_LOG_* values)."""
        ptiff_logger_set_level(int(level))

    def log(self, level: int, message: str) -> None:
        """Emit ``message`` at ``level``; filtered out below the current threshold."""
        ptiff_logger_log(int(level), message)

    def trace(self, message: str) -> None:
        self.log(self.TRACE, message)

    def debug(self, message: str) -> None:
        self.log(self.DEBUG, message)

    def info(self, message: str) -> None:
        self.log(self.INFO, message)

    def warning(self, message: str) -> None:
        self.log(self.WARN, message)

    def error(self, message: str) -> None:
        self.log(self.ERROR, message)

    def critical(self, message: str) -> None:
        self.log(self.CRITICAL, message)
