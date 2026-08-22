//! PTIFF 1.0 Rust core.
//!
//! Fundamentals:
//!   - [`error`]: stable, additive [`ErrorCode`], [`Error`], [`Result`].
//!   - [`pixel_type`]: scalar sample types.
//!   - [`id`]: strongly-typed opaque handles ([`id::TileId`], ...).
//!   - [`image`]: image-domain types ([`image::Image`], [`image::ImageDescriptor`],
//!     [`image::CompressionKind`], [`image::TileInfo`]).
//!   - [`scene`]: the owning container of images ([`scene::Scene`]).
//!   - [`tile`]: tiling grid and tile-mapping types
//!     ([`tile::TileLayout`], [`tile::TileIndex`], [`tile::Tile`], ...).
//!   - [`io`]: byte transport ([`io::BinaryReader`], [`io::BinaryWriter`]),
//!     format-neutral [`io::StorageModel`], and pixel-level [`io::TileProvider`].
//!   - [`geometry`]: spatial domain values and frame semantics
//!     ([`geometry::Vec3`], [`geometry::Quaternion`], [`geometry::Extrinsics`],
//!     [`geometry::Intrinsics`], [`geometry::Frame`], [`geometry::FramePair`]),
//!     built on the `multicalc` math kernel (GEOMETRY-FOUNDATION.md).
//!
//! Per the architectural plan (PTIFF-1.0-RUST-CORE-PLAN.md), this crate is the
//! reference, single-maintained core. It deliberately exposes **no** C ABI and
//! depends on no production feature yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod geometry;
pub mod id;
pub mod image;
pub mod io;
pub mod pixel_type;
pub mod scene;
pub mod tile;

pub use error::{Error, ErrorCode, Result};
pub use geometry::{
    Camera, CoordinateReferenceSystem, Extrinsics, ExtrinsicsMatrix, Frame, FramePair, Geometry,
    GeometryKind, Intrinsics, IntrinsicsMatrix, LensModel, LensModelKind, Planet, Pose, Projection,
    ProjectionKind, ProjectionMatrix, Quaternion, RotationMatrix, Screw, ScrewAxis, ScrewMotion,
    Vec3,
};
pub use id::{AnnotationId, CameraId, GeometryId, ImageId, LayerId, TileId};
pub use image::{CompressionKind, Image, ImageDescriptor, PixelType, TileInfo};
pub use io::{
    BackendCapabilities, BackendFactory, BinaryReader, BinaryWriter, Deserializer, ImageSink,
    ImageSource, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel, TileProvider,
};

pub use scene::Scene;
