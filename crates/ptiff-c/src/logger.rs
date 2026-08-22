//! Logger C ABI (`bindings/c/ptiff_logger.h`).
//!
//! The `ptiff_logger_*` surface mirrors the C++ `ptiff::Logger` singleton and
//! forwards to the dependency-free Rust logger in `ptiff-core::logging`
//! (`LogLevel`). The `log_level` enum ordering matches the C++ `ptiff::LogLevel`
//! and is a hard C-ABI contract (`PTIFF_LOG_TRACE == 0` … `PTIFF_LOG_OFF == 6`).

use std::ffi::CStr;
use std::os::raw::c_char;

use ptiff_core::LogLevel;

/// Mirror of the C `ptiff_log_level` enum (`ptiff_logger.h`). Ordering matches
/// the C++ `ptiff::LogLevel` and the core's [`LogLevel`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ptiff_log_level {
    PTIFF_LOG_TRACE = 0,
    PTIFF_LOG_DEBUG = 1,
    PTIFF_LOG_INFO = 2,
    PTIFF_LOG_WARN = 3,
    PTIFF_LOG_ERROR = 4,
    PTIFF_LOG_CRITICAL = 5,
    PTIFF_LOG_OFF = 6,
}

/// Forward a `ptiff_log_level` int to the core [`LogLevel`]. Out-of-range values
/// clamp to the nearest valid level (matching [`LogLevel::set_level`]).
fn to_core_level(level: i32) -> LogLevel {
    match level {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        4 => LogLevel::Error,
        5 => LogLevel::Critical,
        6 => LogLevel::Off,
        // Same clamp semantics as the core.
        _ if level < 0 => LogLevel::Trace,
        _ => LogLevel::Off,
    }
}

/// Sets the process-wide logger level (C mirror of `ptiff::Logger::setLevel`).
///
/// # Safety
///
/// `level` is a plain `int`; the function neither dereferences nor stores any
/// pointer, so it is always safe to call.
#[no_mangle]
pub extern "C" fn ptiff_logger_set_level(level: i32) {
    LogLevel::set_level(level);
}

/// Returns the current process-wide logger level (C mirror of `ptiff::Logger::level`).
#[no_mangle]
pub extern "C" fn ptiff_logger_level() -> i32 {
    LogLevel::current() as i32
}

/// Logs a message at `level` via the core logger.
///
/// # Safety
///
/// `message` must be null or a valid NUL-terminated C string. A null pointer
/// is treated as an empty message (never dereferenced).
#[no_mangle]
pub extern "C" fn ptiff_logger_log(level: i32, message: *const c_char) {
    let text = if message.is_null() {
        "".to_string()
    } else {
        // SAFETY: caller guarantees `message` is a valid NUL-terminated C string
        // when non-null; `CStr::from_ptr` reads it and copies into an owned String
        // so the borrow never escapes this function.
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    to_core_level(level).log(&text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_surface_is_callable_and_stable() {
        // Set DEBUG → level() reports DEBUG (forwards to the core logger).
        let start = ptiff_logger_level();
        ptiff_logger_set_level(ptiff_log_level::PTIFF_LOG_DEBUG as i32);
        let after = ptiff_logger_level();
        assert_eq!(start, ptiff_log_level::PTIFF_LOG_INFO as i32); // default Info
        assert_eq!(after, ptiff_log_level::PTIFF_LOG_DEBUG as i32);
        // Null message is safe (treated as empty).
        ptiff_logger_log(ptiff_log_level::PTIFF_LOG_INFO as i32, std::ptr::null());
        // Reset to the default so other tests see a clean level.
        ptiff_logger_set_level(ptiff_log_level::PTIFF_LOG_INFO as i32);
        assert_eq!(ptiff_logger_level(), ptiff_log_level::PTIFF_LOG_INFO as i32);
    }

    #[test]
    fn log_level_ordering_matches_c_header() {
        assert_eq!(ptiff_log_level::PTIFF_LOG_TRACE as i32, 0);
        assert_eq!(ptiff_log_level::PTIFF_LOG_OFF as i32, 6);
        // The C ordering must also agree with the core's LogLevel numbering.
        assert_eq!(to_core_level(0), LogLevel::Trace);
        assert_eq!(to_core_level(6), LogLevel::Off);
    }
}
