//! Concrete storage backends, each gated behind its own cargo feature.
//!
//! Mirrors the C++ `ptiff::io::backend` namespace (see
//! `libptiff/include/ptiff/io/backend/`).

pub mod memory_backend;

pub use memory_backend::MemoryBackend;
