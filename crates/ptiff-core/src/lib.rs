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
//!
//! Per the architectural plan (PTIFF-1.0-RUST-CORE-PLAN.md), this crate is the
//! reference, single-maintained core. It deliberately exposes **no** C ABI and
//! depends on no production feature yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod id;
pub mod image;
pub mod io;
pub mod pixel_type;
pub mod scene;
pub mod tile;

pub use error::{Error, ErrorCode, Result};
pub use id::{AnnotationId, CameraId, GeometryId, ImageId, LayerId, TileId};
pub use image::{CompressionKind, Image, ImageDescriptor, PixelType, TileInfo};
pub use io::{BinaryReader, BinaryWriter, StorageModel, TileProvider};
pub use scene::Scene;
