//! Everything needed to construct an image.
//!
//! Mirrors `ptiff::ImageDescriptor` (see
//! `libptiff/include/ptiff/image/image_descriptor.hpp`).

use crate::image::{CompressionKind, PixelType, TileInfo};

/// Everything needed to construct an image.
///
/// A separate struct (rather than a long constructor parameter list) so call
/// sites read as named fields. It is the input to `Scene::add_image` in a later
/// phase.
///
/// `tile_info` and `ground_sample_distance_meters` are optional: a `None` value
/// means "not specified". They are **not** preserved through a TIFF round-trip.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ImageDescriptor {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Sample type of each channel.
    pub pixel_type: PixelType,
    /// Channels per pixel (samples/pixel).
    pub channel_count: u32,
    /// Optional ground-sample distance, in meters.
    pub ground_sample_distance_meters: Option<f64>,
    /// Optional tiling layout in pixels.
    pub tile_info: Option<TileInfo>,
    /// Optional compression scheme.
    pub compression: Option<CompressionKind>,
}

impl ImageDescriptor {
    /// Builds a descriptor with the given dimensions and a degenerate single
    /// channel of [`PixelType::UInt8`], the C++ default.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixel_type: PixelType::UInt8,
            channel_count: 1,
            ground_sample_distance_meters: None,
            tile_info: None,
            compression: None,
        }
    }
}

impl Default for ImageDescriptor {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shape() {
        let d = ImageDescriptor::new(64, 32);
        assert_eq!(d.width, 64);
        assert_eq!(d.height, 32);
        assert_eq!(d.pixel_type, PixelType::UInt8);
        assert_eq!(d.channel_count, 1);
        assert_eq!(d.ground_sample_distance_meters, None);
        assert_eq!(d.tile_info, None);
        assert_eq!(d.compression, None);
    }

    #[test]
    fn fully_specified() {
        let d = ImageDescriptor {
            width: 640,
            height: 480,
            pixel_type: PixelType::UInt16,
            channel_count: 1,
            ground_sample_distance_meters: Some(2.5),
            tile_info: Some(TileInfo::new(256, 256)),
            compression: Some(CompressionKind::Deflate),
        };
        assert_eq!(d.pixel_type, PixelType::UInt16);
        assert_eq!(d.compression, Some(CompressionKind::Deflate));
        assert_eq!(d.tile_info, Some(TileInfo::new(256, 256)));
    }
}
