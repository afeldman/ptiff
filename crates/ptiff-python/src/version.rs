//! Version and backend-registry surface (module-level).

use pyo3::prelude::*;

use crate::constants::PTIFF_ABI_VERSION;

/// The PTIFF runtime version, `(major, minor, patch)`.
///
/// The Rust core is the single implementation, so runtime and compile-time
/// versions always agree.
#[pyfunction]
fn runtime_version() -> (u64, u64, u64) {
    let v = ptiff::APP_VERSION.clone();
    (v.major, v.minor, v.patch)
}

/// The PTIFF compile-time version, `(major, minor, patch)`.
#[pyfunction]
fn compile_time_version() -> (u64, u64, u64) {
    runtime_version()
}

/// The PTIFF version as a `semver` string (e.g. `"1.1.0"`).
#[pyfunction]
fn version_string() -> String {
    ptiff::VERSION_STR.to_string()
}

/// Comma-space-joined list of registered storage backends (e.g.
/// `"tiff, memory"`), matching the C ABI's `ptiff_backend_names()`.
#[pyfunction]
fn backend_names() -> String {
    ptiff::BackendFactory::instance()
        .registered_backends()
        .join(", ")
}

/// The PTIFF ABI break-counter version.
#[pyfunction]
fn abi_version() -> u32 {
    PTIFF_ABI_VERSION
}

/// Python-visible module function bindings.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(runtime_version, m)?)?;
    m.add_function(wrap_pyfunction!(compile_time_version, m)?)?;
    m.add_function(wrap_pyfunction!(version_string, m)?)?;
    m.add_function(wrap_pyfunction!(backend_names, m)?)?;
    m.add_function(wrap_pyfunction!(abi_version, m)?)?;
    Ok(())
}
