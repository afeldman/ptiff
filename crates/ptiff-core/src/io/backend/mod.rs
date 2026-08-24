//! Concrete storage backends, each gated behind its own cargo feature.
//!
//! Mirrors the C++ `ptiff::io::backend` namespace (see
//! `libptiff/include/ptiff/io/backend/`).

// The in-memory ("PMEM") backend, gated behind the `memory-backend` feature.
#[cfg(feature = "memory-backend")]
pub mod memory_backend;
// Dependency-free pixel tier, shared by MemoryBackend and any other backend
// that stores a plain, uncompressed, tile-addressable pixel block (see
// `memory-pixel-tier` / `isis-backend` in Cargo.toml).
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub mod memory_image_sink;
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub mod memory_image_source;
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub mod memory_layout;

// TIFF/BigTIFF backend, gated behind the `tiff-backend` feature. The format
// layer (endian, header, tag, pixel type, IFD) and the writer/sink/source
// layer are dependency-free over the byte transport.
#[cfg(feature = "tiff-backend")]
pub mod tiff;

// USGS ISIS3-style backend, gated behind the `isis-backend` feature.
// Dependency-free; reuses the memory-pixel-tier for pixel I/O.
#[cfg(feature = "isis-backend")]
pub mod isis;

// NASA PDS4-style backend, gated behind the `pds4-backend` feature. Uses
// `quick-xml` for the XML label and reuses the memory-pixel-tier for pixel I/O.
#[cfg(feature = "pds4-backend")]
pub mod pds4;

// Zarr-style chunked array backend, gated behind the `zarr-backend` feature.
// Unlike isis/pds4 it does NOT reuse the memory-pixel-tier: chunks are
// individually zstd/zlib-compressed, so it carries its own ImageSource/
// ImageSink. Uses `serde_json` for the JSON array header and `zstd`/`flate2`
// for chunk compression.
#[cfg(feature = "zarr-backend")]
pub mod zarr;

// OpenEXR storage backend, gated behind the `openexr-backend` feature. Real
// standalone `.exr` files read/written through the pure-Rust `exr` crate
// (no unsafe code). Unlike isis/pds4 it does NOT reuse the memory-pixel-tier:
// the whole image is always one tile written/read as a genuine OpenEXR
// document.
#[cfg(feature = "openexr-backend")]
pub mod openexr;

#[cfg(feature = "memory-backend")]
pub use memory_backend::MemoryBackend;
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub use memory_image_sink::MemoryImageSink;
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub use memory_image_source::MemoryImageSource;
#[cfg(any(feature = "memory-backend", feature = "memory-pixel-tier"))]
pub use memory_layout::{image_info_from_model, image_pixel_offset, MemoryImageInfo};

// TIFF/BigTIFF `StorageBackend` implementation (name "tiff").
#[cfg(feature = "tiff-backend")]
pub use tiff::TiffBackend;

// ISIS3 `StorageBackend` implementation (name "isis").
#[cfg(feature = "isis-backend")]
pub use isis::IsisBackend;

// PDS4 `StorageBackend` implementation (name "pds4").
#[cfg(feature = "pds4-backend")]
pub use pds4::Pds4Backend;

// Zarr `StorageBackend` implementation (name "zarr").
#[cfg(feature = "zarr-backend")]
pub use zarr::ZarrBackend;

// OpenEXR `StorageBackend` implementation (name "openexr").
#[cfg(feature = "openexr-backend")]
pub use openexr::OpenExrBackend;
