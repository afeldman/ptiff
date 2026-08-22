//! Concrete storage backends, each gated behind its own cargo feature.
//!
//! Mirrors the C++ `ptiff::io::backend` namespace (see
//! `libptiff/include/ptiff/io/backend/`).

// The in-memory ("PMEM") backend, gated behind the `memory-backend` feature.
#[cfg(feature = "memory-backend")]
pub mod memory_backend;
#[cfg(feature = "memory-backend")]
pub mod memory_image_sink;
#[cfg(feature = "memory-backend")]
pub mod memory_image_source;
#[cfg(feature = "memory-backend")]
pub mod memory_layout;

// TIFF/BigTIFF backend, gated behind the `tiff-backend` feature. The format
// layer (endian, header, tag, pixel type, IFD) is dependency-free over the byte
// transport; the writer-policy sink/source layer comes in a later phase.
#[cfg(feature = "tiff-backend")]
pub mod tiff;

#[cfg(feature = "memory-backend")]
pub use memory_backend::MemoryBackend;
#[cfg(feature = "memory-backend")]
pub use memory_image_sink::MemoryImageSink;
#[cfg(feature = "memory-backend")]
pub use memory_image_source::MemoryImageSource;
#[cfg(feature = "memory-backend")]
pub use memory_layout::{image_info_from_model, image_pixel_offset, MemoryImageInfo};
