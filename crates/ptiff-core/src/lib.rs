//! PTIFF 1.0 Rust core.
//!
//! Fundamentals:
//!   - [`error`]: stable, additive [`ErrorCode`], [`Error`], [`Result`].
//!   - [`pixel_type`]: scalar sample types.
//!   - [`image`]: image-domain types ([`image::ImageDescriptor`],
//!     [`image::CompressionKind`], [`image::TileInfo`]).
//!   - [`tile`]: tiling grid and tile-mapping types
//!     ([`tile::TileLayout`], [`tile::TileIndex`], ...).
//!
//! Per the architectural plan (PTIFF-1.0-RUST-CORE-PLAN.md), this crate is the
//! reference, single-maintained core. It deliberately exposes **no** C ABI and
//! depends on no production feature yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod image;
pub mod pixel_type;
pub mod tile;

pub use error::{Error, ErrorCode, Result};
pub use image::{CompressionKind, ImageDescriptor, PixelType, TileInfo};
