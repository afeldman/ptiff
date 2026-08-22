//! Logger C ABI (`bindings/c/ptiff_logger.h`).
//!
//! The `ptiff_logger_*` surface mirrors the C++ `ptiff::Logger` singleton. The
//! Rust core has **no logger** yet, so these entry points are recognised stubs:
//! they keep the exact symbol/ABI surface (so the Go/Ruby/Python/Octave runtimes
//! link and set level without a compile-time break), but the operations are
//! no-ops rather than forwarding into a (future) Rust logger. The `log_level`
//! enum ordering matches the C++ `ptiff::LogLevel`.

/// Mirror of the C `ptiff_log_level` enum (`ptiff_logger.h`). Ordering matches
/// the C++ `ptiff::LogLevel`.
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

/// Sets the logger level. Currently a no-op (no Rust logger wired into the
/// C-ABI slice yet).
///
/// # Safety
///
/// `level` is a plain `int`; the function never dereferences any pointer.
#[no_mangle]
pub extern "C" fn ptiff_logger_set_level(level: i32) {
    // No-op until a Rust logger exists; accept and ignore the level so foreign
    // runtimes calling this never break.
    let _ = level;
}

/// Returns the current logger level. The stub does not track state, so it
/// always reports `PTIFF_LOG_OFF` (nothing is being logged until a logger
/// lands).
#[no_mangle]
pub extern "C" fn ptiff_logger_level() -> i32 {
    ptiff_log_level::PTIFF_LOG_OFF as i32
}

/// Logs a message at `level`. Currently a no-op (messages are discarded until a
/// Rust logger is wired in).
///
/// # Safety
///
/// `message` must be null or a valid NUL-terminated C string; the stub never
/// dereferences it (it is ignored), so a null pointer is safe.
#[no_mangle]
pub extern "C" fn ptiff_logger_log(_level: i32, _message: *const std::os::raw::c_char) {
    // No-op: no logger yet.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_surface_is_callable_and_stable() {
        // The stub surface must be ABI-callable (link + no panic) and never
        // report anything but OFF.
        let start = ptiff_logger_level();
        // Plain int + null message are both safe for the stubs.
        ptiff_logger_set_level(ptiff_log_level::PTIFF_LOG_DEBUG as i32);
        ptiff_logger_log(ptiff_log_level::PTIFF_LOG_INFO as i32, std::ptr::null());
        let after = ptiff_logger_level();
        assert_eq!(start, ptiff_log_level::PTIFF_LOG_OFF as i32);
        assert_eq!(after, start);
    }

    #[test]
    fn log_level_ordering_matches_c_header() {
        assert_eq!(ptiff_log_level::PTIFF_LOG_TRACE as i32, 0);
        assert_eq!(ptiff_log_level::PTIFF_LOG_OFF as i32, 6);
    }
}
