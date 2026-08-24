//! Everything needed to construct an image.
//!
//! Mirrors `ptiff::ImageDescriptor` (see
//! `libptiff/include/ptiff/image/image_descriptor.hpp`).

use crate::image::{CompressionKind, PixelType, TileInfo};
use crate::Camera;
use crate::CoordinateReferenceSystem;

/// Everything needed to construct an image.
///
/// A separate struct (rather than a long constructor parameter list) so call
/// sites read as named fields. It is the input to `Scene::add_image` in a later
/// phase.
///
/// `tile_info` and `ground_sample_distance_meters` are optional: a `None` value
/// means "not specified". They are **not** preserved through a TIFF round-trip.
///
/// `camera` and `crs` are the PTIFF extension domains (RFC-caveat, see the
/// `ptiff.<domain>.<key>` marshalling in `crate::geometry`): optional camera
/// calibration/pose and a georeferencing context. When set they are
/// round-tripped through the `ptiff.camera.*` / `ptiff.crs.*` storage-model
/// fields and the private TIFF tags 65002 / 65003.
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
    /// Optional camera calibration / pose (PTIFF extension, tag 65002).
    ///
    /// Excluded from the (diagnostics-only) serde path: the extension domains
    /// round-trip through the `ptiff.*` storage-model fields and TIFF tags, not
    /// through serde JSON.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub camera: Option<Camera>,
    /// Optional coordinate reference system / georeferencing (PTIFF extension,
    /// tag 65003).
    ///
    /// Excluded from the serde path for the same reason as [`Self::camera`];
    /// [`CoordinateReferenceSystem`] itself is not serde because
    /// [`crate::geometry::Frame`] holds a `'static` identifier.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub crs: Option<CoordinateReferenceSystem>,
    /// Optional generic `ptiff.<domain>.<key>` extension metadata (RFC-7002),
    /// for the domains NOT covered by the typed [`Self::camera`] (65002) /
    /// [`Self::crs`] (65003) fields — namely SPICE (65001), scientific layers
    /// (65004), provenance (65005) and any unknown/future `ptiff.*` keys. Keys
    /// are fully-qualified storage-model keys (e.g. `"ptiff.spice.frame"`).
    ///
    /// The `ptiff.camera.*` / `ptiff.crs.*` keys are reserved for the typed
    /// fields above and MUST NOT be present here: those two domains are the
    /// single source of truth and are round-tripped exclusively through
    /// `camera` / `crs`. Entries are stored in ascending key order.
    ///
    /// Excluded from the serde path for the same reason as the other extension
    /// fields; the extension domains round-trip through the `ptiff.*`
    /// storage-model fields and TIFF tags.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub metadata: std::collections::BTreeMap<String, String>,
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
            camera: None,
            crs: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

/// A builder for [`ImageDescriptor`].
///
/// [`ImageDescriptor`] is `#[non_exhaustive]` (so its public fields can be read
/// from any crate but its struct literal cannot be written outside this crate).
/// This builder is the supported construction path: start from
/// [`ImageDescriptorBuilder::new`] (which yields the same degenerate defaults
/// as [`ImageDescriptor::new`]) and override the wanted fields with the
/// fluent setter methods. It mirrors the ergonomics of the pre-1.0
/// `ImageDescriptorBuilder` and is re-exported by the idiomatic `ptiff` crate.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageDescriptorBuilder {
    descriptor: ImageDescriptor,
}

impl ImageDescriptorBuilder {
    /// Starts a builder with `width`/`height` and the degenerate defaults of
    /// [`ImageDescriptor::new`] (`UInt8`, one channel, no tiling/compression,
    /// no camera/CRS).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            descriptor: ImageDescriptor::new(width, height),
        }
    }

    /// Sets the sample type of each channel.
    #[must_use]
    pub fn pixel_type(mut self, pixel_type: PixelType) -> Self {
        self.descriptor.pixel_type = pixel_type;
        self
    }

    /// Sets the number of channels per pixel.
    #[must_use]
    pub fn channel_count(mut self, channel_count: u32) -> Self {
        self.descriptor.channel_count = channel_count;
        self
    }

    /// Sets the optional ground-sample distance in meters.
    #[must_use]
    pub fn ground_sample_distance_meters(mut self, gsd: Option<f64>) -> Self {
        self.descriptor.ground_sample_distance_meters = gsd;
        self
    }

    /// Sets the optional tiling layout in pixels.
    #[must_use]
    pub fn tile_info(mut self, tile: Option<TileInfo>) -> Self {
        self.descriptor.tile_info = tile;
        self
    }

    /// Convenience: sets a square tile extent from its pixel width/height.
    #[must_use]
    pub fn tile(mut self, tile_width: u32, tile_height: u32) -> Self {
        self.descriptor.tile_info = Some(TileInfo::new(tile_width, tile_height));
        self
    }

    /// Sets the optional compression scheme.
    #[must_use]
    pub fn compression(mut self, compression: Option<CompressionKind>) -> Self {
        self.descriptor.compression = compression;
        self
    }

    /// Sets the optional camera calibration / pose (PTIFF extension, 65002).
    #[must_use]
    pub fn camera(mut self, camera: Option<Camera>) -> Self {
        self.descriptor.camera = camera;
        self
    }

    /// Sets the optional coordinate reference system (PTIFF extension, 65003).
    #[must_use]
    pub fn crs(mut self, crs: Option<CoordinateReferenceSystem>) -> Self {
        self.descriptor.crs = crs;
        self
    }

    /// Sets one generic `ptiff.<domain>.<key>` extension-metadata record
    /// (RFC-7002). The key must be fully-qualified
    /// (e.g. `"ptiff.spice.frame"`) and must NOT start with `"ptiff.camera."`
    /// or `"ptiff.crs."` — those two domains are carried by [`Self::camera`] /
    /// [`Self::crs`].
    ///
    /// # Panics
    ///
    /// Panics if `key` is a reserved `ptiff.camera.*` / `ptiff.crs.*` key, or
    /// if it does not start with the `ptiff.` prefix.
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        assert_extension_key(&key);
        self.descriptor.metadata.insert(key, value.into());
        self
    }

    /// Consumes the builder and returns the configured [`ImageDescriptor`].
    #[must_use]
    pub fn build(self) -> ImageDescriptor {
        self.descriptor
    }
}

impl Default for ImageDescriptor {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Validates a fully-qualified `ptiff.<domain>.<key>` extension-metadata key.
///
/// Returns a descriptive error message for the reserved `ptiff.camera.*` /
/// `ptiff.crs.*` keys (which are carried by the typed fields on
/// [`ImageDescriptor`]) or for a key that is not in the `ptiff.<domain>.<key>`
/// shape.
pub fn validate_extension_key(key: &str) -> Result<(), String> {
    let pfx = "ptiff.";
    if !key.starts_with(pfx) {
        return Err(format!(
            "metadata key must be fully qualified (must start with 'ptiff.'): got '{key}'"
        ));
    }
    if key.starts_with("ptiff.camera.") || key.starts_with("ptiff.crs.") {
        return Err(format!(
            "metadata key '{key}' is reserved for the typed camera/CRS fields; use              ImageDescriptor::camera / ImageDescriptor::crs instead"
        ));
    }
    // Require at least `ptiff.<domain>.` (non-empty domain and key).
    let rest = &key[pfx.len()..];
    match rest.find('.') {
        Some(idx) if idx > 0 && idx + 1 < rest.len() => Ok(()),
        _ => Err(format!(
            "metadata key must be of the shape 'ptiff.<domain>.<key>': got '{key}'"
        )),
    }
}

/// Panics on an invalid extension-metadata key; used by the builder setter.
fn assert_extension_key(key: &str) {
    if let Err(msg) = validate_extension_key(key) {
        panic!("ImageDescriptor::metadata: {msg}");
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
        assert_eq!(d.camera, None);
        assert_eq!(d.crs, None);
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
            ..ImageDescriptor::default()
        };
        assert_eq!(d.pixel_type, PixelType::UInt16);
        assert_eq!(d.compression, Some(CompressionKind::Deflate));
        assert_eq!(d.tile_info, Some(TileInfo::new(256, 256)));
        assert_eq!(d.camera, None);
        assert_eq!(d.crs, None);
    }

    #[test]
    fn builder_defaults_and_overrides() {
        let d = ImageDescriptorBuilder::new(128, 64)
            .pixel_type(PixelType::Float32)
            .channel_count(3)
            .ground_sample_distance_meters(Some(1.0))
            .tile(16, 16)
            .compression(Some(CompressionKind::Lzw))
            .build();
        assert_eq!(d.width, 128);
        assert_eq!(d.height, 64);
        assert_eq!(d.pixel_type, PixelType::Float32);
        assert_eq!(d.channel_count, 3);
        assert_eq!(d.ground_sample_distance_meters, Some(1.0));
        assert_eq!(d.tile_info, Some(TileInfo::new(16, 16)));
        assert_eq!(d.compression, Some(CompressionKind::Lzw));
        assert_eq!(d.camera, None);
        assert_eq!(d.crs, None);
    }

    #[test]
    fn builder_starts_from_degenerate_defaults() {
        let d = ImageDescriptorBuilder::new(10, 20).build();
        assert_eq!(d.pixel_type, PixelType::UInt8);
        assert_eq!(d.channel_count, 1);
        assert_eq!(d.tile_info, None);
        assert_eq!(d.compression, None);
    }

    #[test]
    fn builder_metadata_accumulates_and_is_sorted() {
        let d = ImageDescriptorBuilder::new(16, 16)
            .metadata("ptiff.provenance.software", "libptiff")
            .metadata("ptiff.spice.frame", "IAU_MOON")
            .build();
        // The map stores keys in ascending byte order.
        let keys: Vec<&String> = d.metadata.keys().collect();
        assert_eq!(
            keys,
            vec![
                &"ptiff.provenance.software".to_string(),
                &"ptiff.spice.frame".to_string()
            ]
        );
        assert_eq!(
            d.metadata.get("ptiff.spice.frame").map(String::as_str),
            Some("IAU_MOON")
        );
        assert_eq!(
            d.metadata
                .get("ptiff.provenance.software")
                .map(String::as_str),
            Some("libptiff")
        );
    }

    #[test]
    fn validate_extension_key_rules() {
        // Valid fully-qualified keys.
        assert!(validate_extension_key("ptiff.spice.frame").is_ok());
        assert!(validate_extension_key("ptiff.provenance.software").is_ok());
        assert!(validate_extension_key("ptiff.layers.dem").is_ok());
        // Reserved typed domains are rejected.
        assert!(validate_extension_key("ptiff.camera.model").is_err());
        assert!(validate_extension_key("ptiff.crs.planet_name").is_err());
        // Non-ptiff / malformed keys are rejected.
        assert!(validate_extension_key("spice.frame").is_err());
        assert!(validate_extension_key("ptiff.").is_err());
        assert!(validate_extension_key("ptiff.spice").is_err());
        // Empty domain or empty key is rejected.
        assert!(validate_extension_key("ptiff..frame").is_err());
        assert!(validate_extension_key("ptiff.spice.").is_err());
    }
}
