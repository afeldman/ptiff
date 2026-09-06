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
pub use ptiff_core::{lens_model_kind_from_str, lens_model_kind_str};
/// Core domain types re-exported at the crate root so callers write
/// `ptiff::Scene` rather than reaching into `ptiff_core`. Includes the
/// geometry, image, scene, pixel, tile and IO model types.
pub use ptiff_core::{
    AnnotationId, BackendCapabilities, BackendFactory, BinaryReader, BinaryWriter, Camera,
    CameraId, CompressionKind, CoordinateReferenceSystem, Ellipsoid, Error, ErrorCode, Extrinsics,
    Frame, FramePair, Geometry, GeometryId, GeometryKind, Image, ImageDescriptor,
    ImageDescriptorBuilder, ImageId, Intrinsics, LayerId, LensModel, LensModelKind, PixelType,
    Planet, Pose, Projection, ProjectionKind, Quaternion, Result, Scene, StorageModel, TileInfo,
    Vec3,
};

/// Runtime-queryable crate version (e.g. for a `ptiff --version` CLI flag).
///
/// [`APP_VERSION`] is the parsed, comparable `semver::Version` (use
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

#[cfg(feature = "remote")]
mod remote_tiff;

#[cfg(feature = "remote")]
pub use remote_tiff::RemoteTiff;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constants_are_queryable() {
        // The crate version is available for a `ptiff --version` CLI flag:
        // `VERSION_STR` mirrors Cargo.toml's `version` and `APP_VERSION`
        // parses it into a comparable semver. The parsed `major` must equal
        // the Cargo version's major so the linkage constant always matches the
        // compiled crate (regardless of which major it happens to be).
        assert_eq!(VERSION_STR, env!("CARGO_PKG_VERSION"));
        assert_eq!(APP_VERSION.to_string(), VERSION_STR);
        let cargo_major: u64 = env!("CARGO_PKG_VERSION_MAJOR")
            .parse()
            .expect("major parses");
        assert_eq!(APP_VERSION.major, cargo_major);
    }

    #[test]
    fn scene_level_camera_and_geometry_via_facade() {
        // The idiomatic crate re-exports Scene + Camera + Geometry + the ID
        // types, so scene-level cameras/geometries are usable from `ptiff`.
        use crate::{Camera, CameraId, Extrinsics, Geometry, GeometryId, GeometryKind, Intrinsics};

        let mut scene = Scene::new();
        let cam_id = scene.add_camera(Camera::from_model(
            "pinhole",
            Intrinsics::new(400.0, 401.0, 32.0, 32.0),
            Extrinsics::default(),
            "2026-08-21T10:00:00Z",
        ));
        let geom_id = scene.add_geometry(Geometry::new(GeometryKind::Unspecified));

        // The facade type aliases are the same handles the scene returns.
        let typed: CameraId = cam_id;
        let typed_geom: GeometryId = geom_id;
        let cam = scene.camera(typed).unwrap();
        assert_eq!(cam.model_name(), "pinhole");
        assert!(scene.geometry(typed_geom).is_ok());
        assert_eq!(scene.camera_count(), 1);
        assert_eq!(scene.geometry_count(), 1);
    }
}
