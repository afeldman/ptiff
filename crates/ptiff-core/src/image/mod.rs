//! Image-domain types.
//!
//! `Image` is defined directly in this module (mirrors `ptiff::Image`), along
//! with the value types it composes.

pub mod compression_kind;
pub mod image_descriptor;
pub mod tile_info;

pub use compression_kind::CompressionKind;
pub use image_descriptor::{ImageDescriptor, ImageDescriptorBuilder};
pub use tile_info::TileInfo;

// Re-export for convenience: `ptiff::image::PixelType`.
pub use crate::pixel_type::PixelType;

// --- Image (defined directly in this module to avoid `image::image` inception) ---

/// A single scientific raster image: geometry and storage metadata, not pixel
/// data.
///
/// Owns the descriptive metadata of one image (dimensions, pixel type, channel
/// count, optional ground sampling distance, tile layout and compression
/// scheme). It does **not** hold pixel data — those live in the storage backend
/// and are accessed via the I/O layer.
///
/// There is no `id()` accessor — `Image` identity is scoped to whichever
/// [`crate::Scene`] it was added to (see `Scene::add_image`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Image {
    descriptor: ImageDescriptor,
}

impl Image {
    /// Constructs an image from its metadata descriptor.
    #[must_use]
    pub fn new(descriptor: ImageDescriptor) -> Self {
        Self { descriptor }
    }

    /// Returns the image width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.descriptor.width
    }

    /// Returns the image height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.descriptor.height
    }

    /// Returns the sample type of each channel.
    #[must_use]
    pub const fn pixel_type(&self) -> PixelType {
        self.descriptor.pixel_type
    }

    /// Returns the number of channels per pixel.
    #[must_use]
    pub const fn channel_count(&self) -> u32 {
        self.descriptor.channel_count
    }

    /// Returns the optional ground-sample distance in meters, if specified.
    #[must_use]
    pub const fn ground_sample_distance_meters(&self) -> Option<f64> {
        self.descriptor.ground_sample_distance_meters
    }

    /// Returns the optional tile layout in pixels, if specified.
    #[must_use]
    pub const fn tile_info(&self) -> Option<TileInfo> {
        self.descriptor.tile_info
    }

    /// Returns the optional compression scheme, if specified.
    #[must_use]
    pub const fn compression(&self) -> Option<CompressionKind> {
        self.descriptor.compression
    }

    /// Returns the optional camera calibration / pose (PTIFF extension).
    #[must_use]
    pub const fn camera(&self) -> Option<&crate::Camera> {
        self.descriptor.camera.as_ref()
    }

    /// Returns the optional coordinate reference system (PTIFF extension).
    #[must_use]
    pub const fn crs(&self) -> Option<&crate::CoordinateReferenceSystem> {
        self.descriptor.crs.as_ref()
    }

    /// Returns the generic `ptiff.<domain>.<key>` extension metadata
    /// (RFC-7002), stored in ascending key order. This covers the SPICE
    /// (65001), scientific-layers (65004) and provenance (65005) domains, plus
    /// any unknown/future `ptiff.*` keys carried by the file. The
    /// camera/CRS domains are *not* included here — they are exposed via
    /// [`Image::camera`](Self::camera) / [`Image::crs`](Self::crs).
    #[must_use]
    pub fn metadata(&self) -> &std::collections::BTreeMap<String, String> {
        &self.descriptor.metadata
    }

    /// Returns the value of one generic `ptiff.<domain>.<key>` extension
    /// metadata record, if present.
    #[must_use]
    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.descriptor.metadata.get(key).map(String::as_str)
    }
}

impl From<ImageDescriptor> for Image {
    fn from(descriptor: ImageDescriptor) -> Self {
        Self::new(descriptor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_exposes_descriptor_metadata() {
        let descriptor = ImageDescriptor {
            width: 128,
            height: 64,
            pixel_type: PixelType::UInt8,
            channel_count: 3,
            ground_sample_distance_meters: Some(1.5),
            tile_info: Some(TileInfo::new(16, 16)),
            compression: Some(CompressionKind::Deflate),
            ..ImageDescriptor::default()
        };
        let image = Image::new(descriptor);

        assert_eq!(image.width(), 128);
        assert_eq!(image.height(), 64);
        assert_eq!(image.pixel_type(), PixelType::UInt8);
        assert_eq!(image.channel_count(), 3);
        assert_eq!(image.ground_sample_distance_meters(), Some(1.5));
        assert_eq!(image.tile_info(), Some(TileInfo::new(16, 16)));
        assert_eq!(image.compression(), Some(CompressionKind::Deflate));
    }

    #[test]
    fn image_defaults_match_descriptor_defaults() {
        let image = Image::new(ImageDescriptor::new(64, 32));
        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 32);
        assert_eq!(image.pixel_type(), PixelType::UInt8);
        assert_eq!(image.channel_count(), 1);
        assert_eq!(image.tile_info(), None);
        assert_eq!(image.compression(), None);
    }
}
