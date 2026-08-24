//! Minimal, dependency-free logging facility.
//!
//! The core ships a deliberately small logger that mirrors the C++ `ptiff::Logger`
//! singleton surface (the enum ordering is a hard C-ABI contract; `ptiff-c`
//! cbindgen-authorises it into `target/ptiff_c.h`): a [`LogLevel`] enum with
//! the exact C ordering and a process-wide current level.
//!
//! The ordering constraint is a **C-ABI contract**: the logger surface of
//! `ptiff-c` (and the generated header) must agree that `TRACE == 0` … `OFF == 6`.
//! Keep the variants in
//! this order and do not renumber them.
//!
//! This module is dependency-free (`std` only) and `#![forbid(unsafe_code)]`-safe: the
//! current level lives behind a `std::sync::atomic`. `log` formats a plain line to
//! stderr when the level enables it. A formatting callback is intentionally out of
//! scope for the 1.0 slice.

use std::fmt;
use std::sync::atomic::{AtomicI32, Ordering};

/// Log severity, mirroring the C `ptiff_log_level` ordering.
///
/// **Do not reorder or renumber**: the numeric values are part of the C ABI
/// (see `ptiff-c`, cbindgen emits them into `target/ptiff_c.h`), where
/// `PTIFF_LOG_TRACE == 0` … `PTIFF_LOG_OFF == 6`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum LogLevel {
    /// Finest-grained diagnostic detail.
    Trace = 0,
    /// Detailed diagnostic information.
    Debug = 1,
    /// Normal informational messages.
    Info = 2,
    /// Recoverable conditions worth surfacing.
    Warn = 3,
    /// An error that still allows continuing.
    Error = 4,
    /// A critical, likely fatal, failure.
    Critical = 5,
    /// Logging disabled entirely.
    Off = 6,
}

impl LogLevel {
    /// The current process-wide log level (defaults to [`LogLevel::Info`]).
    ///
    /// This matches the C++ `Logger::level()` "report current level" contract.
    pub fn current() -> LogLevel {
        match LEVEL.load(Ordering::Relaxed) {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            5 => LogLevel::Critical,
            _ => LogLevel::Off,
        }
    }

    /// Sets the process-wide log level (accepts any `i32`; out-of-range values clamp
    /// to the nearest valid level, [`LogLevel::Trace`] / [`LogLevel::Off`], mirroring
    /// a permissive C wrapper).
    pub fn set_level(level: i32) {
        let clamped = level.clamp(0, 6);
        LEVEL.store(clamped, Ordering::Relaxed);
    }

    /// Emits `message` to stderr when `self` is at least as severe as [`LogLevel::current`].
    ///
    /// When logging is disabled (`current() == Off`) the call is a no-op.
    pub fn log(self, message: &str) {
        let current = LogLevel::current();
        if current == LogLevel::Off || self < current {
            return;
        }
        // `format!`/`eprintln!` is used for its panic-safe string handling and to
        // keep this module dependency-free. A non-empty message is always meaningful.
        #[allow(clippy::print_stderr)]
        {
            eprintln!("[ptiff:{}] {}", self.label(), message);
        }
    }

    /// A short, lowercase label for display (used by `log`).
    fn label(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Critical => "CRITICAL",
            LogLevel::Off => "OFF",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Process-wide current log level; default [`LogLevel::Info`] (`2`).
static LEVEL: AtomicI32 = AtomicI32::new(2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_c_ordering_is_preserved() {
        // This exact ordering/numbering is the C ABI contract (ptiff_logger.h) and
        // the `ptiff-c` mirror must keep agreeing with it.
        assert_eq!(LogLevel::Trace as i32, 0);
        assert_eq!(LogLevel::Debug as i32, 1);
        assert_eq!(LogLevel::Info as i32, 2);
        assert_eq!(LogLevel::Warn as i32, 3);
        assert_eq!(LogLevel::Error as i32, 4);
        assert_eq!(LogLevel::Critical as i32, 5);
        assert_eq!(LogLevel::Off as i32, 6);
    }

    #[test]
    fn set_level_and_level_round_trip() {
        LogLevel::set_level(LogLevel::Debug as i32);
        assert_eq!(LogLevel::current(), LogLevel::Debug);
        LogLevel::set_level(LogLevel::Critical as i32);
        assert_eq!(LogLevel::current(), LogLevel::Critical);
        // Reset to the default for other tests.
        LogLevel::set_level(LogLevel::Info as i32);
        assert_eq!(LogLevel::current(), LogLevel::Info);
    }

    #[test]
    fn out_of_range_levels_clamp_to_bounds() {
        // Values outside [0..6] clamp to the nearest valid level (Trace / Off),
        // mirroring a permissive C wrapper that never rejects the input.
        LogLevel::set_level(999);
        assert_eq!(LogLevel::current(), LogLevel::Off);
        LogLevel::set_level(-5);
        assert_eq!(LogLevel::current(), LogLevel::Trace);
        LogLevel::set_level(LogLevel::Info as i32);
    }
}
