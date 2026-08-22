//! Concrete storage backends, each gated behind its own cargo feature.
//!
//! Mirrors the C++ `ptiff::io::backend` namespace (see
//! `libptiff/include/ptiff/io/backend/`).

pub mod memory_backend;
pub mod memory_image_sink;
pub mod memory_image_source;
pub mod memory_layout;

pub use memory_backend::MemoryBackend;
pub use memory_image_sink::MemoryImageSink;
pub use memory_image_source::MemoryImageSource;
pub use memory_layout::{image_info_from_model, image_pixel_offset, MemoryImageInfo};
