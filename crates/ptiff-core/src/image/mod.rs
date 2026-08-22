//! Image-domain types.

pub mod compression_kind;
pub mod image_descriptor;
pub mod tile_info;

pub use compression_kind::CompressionKind;
pub use image_descriptor::ImageDescriptor;
pub use tile_info::TileInfo;

// Re-export for convenience: `ptiff::image::PixelType`.
pub use crate::pixel_type::PixelType;
