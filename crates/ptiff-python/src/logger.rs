//! Process-wide logging (module-level `Logger` class, mirrors the core's
//! dependency-free logger and the SWIG `ptiff.logger` layer).
//!
//! The underlying core logger is a single process-wide instance, so these
//! methods read and mutate that one instance regardless of how many `Logger`
//! objects exist.

use pyo3::prelude::*;

use crate::constants::{
    PTIFF_LOG_CRITICAL, PTIFF_LOG_DEBUG, PTIFF_LOG_ERROR, PTIFF_LOG_INFO, PTIFF_LOG_OFF,
    PTIFF_LOG_TRACE, PTIFF_LOG_WARN,
};

/// A thin, stateless view over the library's global logger.
#[pyclass(module = "ptiff_pyo3", name = "Logger")]
pub struct Logger;

#[pymethods]
impl Logger {
    /// The current minimum level that gets emitted (a `PTIFF_LOG_*` value).
    #[staticmethod]
    fn level() -> u32 {
        ptiff_core::LogLevel::current() as u32
    }

    /// Set the minimum level that gets emitted (a `PTIFF_LOG_*` value).
    #[staticmethod]
    #[pyo3(signature = (level))]
    fn set_level(level: u32) {
        ptiff_core::LogLevel::set_level(level as i32);
    }

    /// Emit `message` at `level`; filtered out below the current threshold.
    #[staticmethod]
    #[pyo3(signature = (level, message))]
    fn log(level: u32, message: &str) {
        crate::logger::level_from_u32(level).log(message);
    }

    #[staticmethod]
    fn trace(message: &str) {
        ptiff_core::LogLevel::Trace.log(message);
    }
    #[staticmethod]
    fn debug(message: &str) {
        ptiff_core::LogLevel::Debug.log(message);
    }
    #[staticmethod]
    fn info(message: &str) {
        ptiff_core::LogLevel::Info.log(message);
    }
    #[staticmethod]
    fn warning(message: &str) {
        ptiff_core::LogLevel::Warn.log(message);
    }
    #[staticmethod]
    fn error(message: &str) {
        ptiff_core::LogLevel::Error.log(message);
    }
    #[staticmethod]
    fn critical(message: &str) {
        ptiff_core::LogLevel::Critical.log(message);
    }
}

/// Python-visible helper: register module-level level constants on `Logger`.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let cls = m.getattr("Logger")?;
    cls.setattr("TRACE", PTIFF_LOG_TRACE)?;
    cls.setattr("DEBUG", PTIFF_LOG_DEBUG)?;
    cls.setattr("INFO", PTIFF_LOG_INFO)?;
    cls.setattr("WARN", PTIFF_LOG_WARN)?;
    cls.setattr("ERROR", PTIFF_LOG_ERROR)?;
    cls.setattr("CRITICAL", PTIFF_LOG_CRITICAL)?;
    cls.setattr("OFF", PTIFF_LOG_OFF)?;
    Ok(())
}

/// Maps a `PTIFF_LOG_*` code to the core `LogLevel`, clamping out-of-range
/// values to `Off` (mirrors the C ABI's permissive handling).
pub(crate) fn level_from_u32(code: u32) -> ptiff_core::LogLevel {
    match code {
        0 => ptiff_core::LogLevel::Trace,
        1 => ptiff_core::LogLevel::Debug,
        2 => ptiff_core::LogLevel::Info,
        3 => ptiff_core::LogLevel::Warn,
        4 => ptiff_core::LogLevel::Error,
        5 => ptiff_core::LogLevel::Critical,
        _ => ptiff_core::LogLevel::Off,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_mapping_covers_core_order() {
        assert_eq!(level_from_u32(0), ptiff_core::LogLevel::Trace);
        assert_eq!(level_from_u32(1), ptiff_core::LogLevel::Debug);
        assert_eq!(level_from_u32(2), ptiff_core::LogLevel::Info);
        assert_eq!(level_from_u32(3), ptiff_core::LogLevel::Warn);
        assert_eq!(level_from_u32(4), ptiff_core::LogLevel::Error);
        assert_eq!(level_from_u32(5), ptiff_core::LogLevel::Critical);
        assert_eq!(level_from_u32(6), ptiff_core::LogLevel::Off);
        // Out-of-range clamps to Off (permissive, like the C wrapper).
        assert_eq!(level_from_u32(99), ptiff_core::LogLevel::Off);
    }
}
