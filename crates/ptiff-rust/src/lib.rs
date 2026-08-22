//! # `ptiff` — idiomatic Rust API over the PTIFF 1.0 core
//!
//! This crate is the friendly, ergonomic frontend over
//! `ptiff-core` (the single maintained core; see
//! [`PTIFF-1.0-RUST-CORE-PLAN.md`](https://github.com/PTIFF/ptiff)). It adds
//! **no** C ABI and **no** C++: normal Rust callers get typed, documented,
//! failure-safe entry points for reading a PTIFF/TIFF/BigTIFF file without
//! touching the core's raw `BinaryReader`/`BinaryWriter` and string-based
//! storage-model fields.
//!
//! # Design
//!
//! - **Thin and dependency-light.** Only `ptiff-core` plus `std` (file I/O).
//!   The idiomatic layer re-exports the core's domain types and adds ergonomic
//!   high-level gateways on top of the core's `StorageBackend`/`Serializer`
//!   traits.
//! - **Everything returns [`Result`].** No panics on malformed input; every
//!   fallible op yields the core's [`Error`]/[`ErrorCode`].
//! - **`forbid(unsafe_code)`.** Same guarantee as the core.
//!
//! # Quick start
//!
//! ```no_run
//! use ptiff::prelude::*;
//!
//! // Open a PTIFF/BigTIFF file and inspect its scene's images.
//! let tiff = ptiff::Tiff::open("image.ptiff").expect("read file");
//! for image in tiff.images() {
//!     println!(
//!         "{}x{} {}x{}",
//!         image.width(),
//!         image.height(),
//!         image.channel_count(),
//!         image.pixel_type()
//!     );
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileLayout, TileRegion};
/// Core domain types re-exported at the crate root so callers write
/// `ptiff::Scene` rather than reaching into `ptiff_core`. Includes the
/// geometry, image, scene, pixel, tile and IO model types.
pub use ptiff_core::{
    BackendCapabilities, BackendFactory, BinaryReader, BinaryWriter, Camera, CompressionKind,
    CoordinateReferenceSystem, Ellipsoid, Error, ErrorCode, Extrinsics, Frame, FramePair, Geometry,
    GeometryKind, Image, ImageDescriptor, ImageDescriptorBuilder, Intrinsics, LensModel, PixelType,
    Planet, Pose, Projection, ProjectionKind, Quaternion, Result, Scene, StorageModel, TileInfo,
    Vec3,
};

/// Runtime-queryable crate version (e.g. for a `ptiff --version` CLI flag).
///
/// [`APP_VERSION`] is the parsed, comparable [`semver::Version`] (use
/// `.major`/`.minor`/`.patch` comparisons); [`VERSION_STR`] is the raw
/// `"x.y.z"` string from `Cargo.toml`, best for display/logging.
pub use ptiff_core::{APP_VERSION, VERSION_STR};

/// Convenience re-exports for `use ptiff::prelude::*;`.
pub mod prelude {
    pub use crate::{
        Camera, CompressionKind, Error, Image, ImageDescriptor, PixelType, Result, Scene, Tiff,
        Tile, TileIndex, TileLayout, TileRegion,
    };
}

mod tiff;

pub use tiff::Tiff;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constants_are_queryable() {
        // The crate version is available for a future `ptiff --version` CLI
        // flag: `VERSION_STR` mirrors Cargo.toml's `version` and `APP_VERSION`
        // parses it into a comparable semver.
        assert_eq!(VERSION_STR, env!("CARGO_PKG_VERSION"));
        assert_eq!(APP_VERSION.to_string(), VERSION_STR);
        assert_eq!(APP_VERSION.major, 0);
    }
}
