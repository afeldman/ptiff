//! # ptiff_pyo3 — PyO3 Python binding over the PTIFF 1.0 Rust core
//!
//! Type-safe, NumPy-integrated Python binding (Plan §9 Option B). It calls the
//! idiomatic `ptiff`/`ptiff-core` crates directly — no SWIG/ctypes round-trip
//! through the C ABI.
//!
//! ```python
//! import ptiff_pyo3 as ptiff
//! import numpy as np
//! with ptiff.open("scene.tif") as doc:
//!     img = doc.image(0)
//!     tile = img.read_tile(column=1, row=0)   # np.ndarray (h, w, channels)
//! ```
//!
//! The `numpy` (rust-numpy, `numpy::PyArray`) crate provides the zero-cost
//! tile array path; `Image::read_tile` returns a copy-constructed
//! `numpy.ndarray` of the image's sample dtype and shape.
// The PyO3/NumPy boundary is FFI by definition, so a minimal, carefully-scoped
// set of `unsafe` blocks is necessary (tile buffer casting / contiguous NumPy
// fill into the numpy crate's global class system). Each is annotated with a
// Safety comment.
#![allow(clippy::unused_unit)]

pub mod camera;
pub mod constants;
pub mod document;
pub mod logger;
pub mod metadata;
pub mod version;
pub mod writer;

use pyo3::prelude::*;

/// The `ptiff_pyo3` extension module.
#[pymodule]
fn ptiff_pyo3(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Module-level functions.
    version::register(m)?;
    constants::register_constants(m)?;

    // Classes.
    m.add_class::<document::Document>()?;
    m.add_class::<document::Image>()?;
    m.add_class::<camera::Camera>()?;
    m.add_class::<metadata::Metadata>()?;
    m.add_class::<logger::Logger>()?;
    m.add_class::<writer::Sink>()?;

    // Module-level `open` / `create_image`.
    m.add_function(wrap_pyfunction!(document::open, m)?)?;
    m.add_function(wrap_pyfunction!(writer::create_image, m)?)?;

    // Level constants on `Logger`.
    logger::register(m)?;

    Ok(())
}
