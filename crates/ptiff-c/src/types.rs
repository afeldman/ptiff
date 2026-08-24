//! FFI-safe value types shared by the C-ABI surface and the Core-type
//! marshalling helpers.
//!
//! These #[repr(C)] structs are emitted into `target/ptiff_c.h` by cbindgen
//! because they cross the ABI boundary by value. They are deliberately
//! C-shaped (plain fields, no lifetimes) so every FFI runtime
//! reads them with the same layout as the C header describes.

#![allow(non_camel_case_types)]

use ptiff::{CompressionKind, Image, ImageDescriptor, PixelType, TileInfo, TileLayout};

/// Mirror of the C `ptiff_pixel_type` enum (`ptiff_image_bridge.h`). Ordering
/// matches `ptiff::PixelType` (UInt8=0 … Float64=4), documented in the header.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ptiff_pixel_type {
    PTIFF_PIXEL_UINT8 = 0,
    PTIFF_PIXEL_UINT16 = 1,
    PTIFF_PIXEL_UINT32 = 2,
    PTIFF_PIXEL_FLOAT32 = 3,
    PTIFF_PIXEL_FLOAT64 = 4,
}

/// Mirror of the C `ptiff_compression_kind` enum (`ptiff_image_bridge.h`).
/// Ordering matches `ptiff::CompressionKind` (None=0 … Jpeg=3).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ptiff_compression_kind {
    PTIFF_COMPRESSION_NONE = 0,
    PTIFF_COMPRESSION_LZW = 1,
    PTIFF_COMPRESSION_DEFLATE = 2,
    PTIFF_COMPRESSION_JPEG = 3,
}

/// Mirror of the C `ptiff_tile_info` struct (`ptiff_image_bridge.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ptiff_tile_info {
    pub tile_width: u32,
    pub tile_height: u32,
}

impl From<TileInfo> for ptiff_tile_info {
    fn from(t: TileInfo) -> Self {
        ptiff_tile_info {
            tile_width: t.tile_width,
            tile_height: t.tile_height,
        }
    }
}

/// Mirror of the C `ptiff_image_descriptor` struct (`ptiff_image_bridge.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ptiff_image_descriptor {
    pub width: u32,
    pub height: u32,
    pub pixel_type: i32,
    pub channel_count: u32,
    pub has_gsd: i32,
    pub gsd: f64,
    pub has_tile_info: i32,
    pub tile_info: ptiff_tile_info,
    pub has_compression: i32,
    pub compression: i32,
}

impl ptiff_image_descriptor {
    /// Builds the empty/false-flavoured descriptor a C caller initialises then
    /// fills in (match the header's documented usage `ptiff_image_descriptor d{};`).
    #[allow(clippy::new_without_default)]
    #[inline]
    pub(crate) fn c_default() -> Self {
        // The C header uses `d{}` value-initialisation; mirror the bit layout.
        ptiff_image_descriptor {
            width: 0,
            height: 0,
            pixel_type: ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32,
            channel_count: 0,
            has_gsd: 0,
            gsd: 0.0,
            has_tile_info: 0,
            tile_info: ptiff_tile_info {
                tile_width: 0,
                tile_height: 0,
            },
            has_compression: 0,
            compression: ptiff_compression_kind::PTIFF_COMPRESSION_NONE as i32,
        }
    }
}

/// Maps a `ptiff_image_descriptor` (from the C side) into a core
/// [`ImageDescriptor`], preserving the optional GSD / tile / compression.
///
/// Camera/CRS extension domains are deliberately left `None`: the C ABI does
/// not (yet) carry them in the plain descriptor struct, and the round-trip /
/// structured marshalling is handled by the `_camera` entry point in a later
/// slice. Optional fields that are unset in the C struct (`has_* == 0`) map to
/// `None`, mirroring the C++ `ptiff::ImageDescriptor` semantics.
pub fn descriptor_from_c(desc: &ptiff_image_descriptor) -> ImageDescriptor {
    let mut d = ImageDescriptor::new(desc.width, desc.height);
    d.pixel_type = pixel_type_from_c(desc.pixel_type);
    d.channel_count = desc.channel_count;
    d.ground_sample_distance_meters = if desc.has_gsd != 0 {
        Some(desc.gsd)
    } else {
        None
    };
    d.tile_info = if desc.has_tile_info != 0 {
        Some(TileInfo::new(
            desc.tile_info.tile_width,
            desc.tile_info.tile_height,
        ))
    } else {
        None
    };
    d.compression = if desc.has_compression != 0 {
        Some(compression_from_c(desc.compression))
    } else {
        None
    };
    d
}

/// Marshals a core [`ImageDescriptor`] into the C-shaped struct, mapping every
/// optional field to a `has_*` flag + value (matching `libptiff_c`'s
/// representation of an optional as a presence flag plus payload).
pub fn descriptor_to_c(d: &ImageDescriptor) -> ptiff_image_descriptor {
    let mut out = ptiff_image_descriptor::c_default();
    out.width = d.width;
    out.height = d.height;
    out.pixel_type = pixel_type_to_c(d.pixel_type) as i32;
    out.channel_count = d.channel_count;
    if let Some(gsd) = d.ground_sample_distance_meters {
        out.has_gsd = 1;
        out.gsd = gsd;
    }
    if let Some(tile) = d.tile_info {
        out.has_tile_info = 1;
        out.tile_info = ptiff_tile_info {
            tile_width: tile.tile_width,
            tile_height: tile.tile_height,
        };
    }
    if let Some(c) = d.compression {
        out.has_compression = 1;
        out.compression = compression_to_c(c) as i32;
    }
    out
}

/// Marshals a core [`Image`] (a scene image) into the C-shaped descriptor, using
/// the image's accessor methods — the scene container is not an `ImageDescriptor`
/// literal. Shared by the read-side C ABI (`ptiff_source_descriptor`,
/// `ptiff_open_path`).
pub fn image_to_c(img: &Image) -> ptiff_image_descriptor {
    let mut out = ptiff_image_descriptor::c_default();
    out.width = img.width();
    out.height = img.height();
    out.pixel_type = pixel_type_to_c(img.pixel_type()) as i32;
    out.channel_count = img.channel_count();
    if let Some(gsd) = img.ground_sample_distance_meters() {
        out.has_gsd = 1;
        out.gsd = gsd;
    }
    if let Some(tile) = img.tile_info() {
        out.has_tile_info = 1;
        out.tile_info = ptiff_tile_info {
            tile_width: tile.tile_width,
            tile_height: tile.tile_height,
        };
    }
    if let Some(c) = img.compression() {
        out.has_compression = 1;
        out.compression = compression_to_c(c) as i32;
    }
    out
}

/// Reconstructs the tile info for a read descriptor from the backend's tile
/// layout, mirroring the C++ `ptiff_metadata.cpp` behaviour (which reads the
/// tiling straight out of the opened image source, not the scene's
/// `ImageDescriptor`). The Rust core's `SceneDeserializer` deliberately leaves
/// `tile_info` unset on deserialization (the canonical schema does not carry
/// it), so without this the C-ABI would report `has_tile_info = 0` for every
/// tiled file it reads back -- diverging from the C++ oracle. We only tag a
/// layout as tiled when it actually is (`is_untiled() == false`), so a
/// single-strip image keeps `has_tile_info = 0` (consistent with the
/// `ptiff_sink` contract that tiled I/O requires a tile layout).
pub fn apply_layout_tile_info(layout: &TileLayout, out: &mut ptiff_image_descriptor) {
    if !layout.is_untiled() {
        out.has_tile_info = 1;
        out.tile_info = ptiff_tile_info {
            tile_width: layout.tile_size.width,
            tile_height: layout.tile_size.height,
        };
    }
}

/// Core [`PixelType`] -> C enum value.
pub fn pixel_type_to_c(p: PixelType) -> ptiff_pixel_type {
    match p {
        PixelType::UInt8 => ptiff_pixel_type::PTIFF_PIXEL_UINT8,
        PixelType::UInt16 => ptiff_pixel_type::PTIFF_PIXEL_UINT16,
        PixelType::UInt32 => ptiff_pixel_type::PTIFF_PIXEL_UINT32,
        PixelType::Float32 => ptiff_pixel_type::PTIFF_PIXEL_FLOAT32,
        PixelType::Float64 => ptiff_pixel_type::PTIFF_PIXEL_FLOAT64,
        // The core list is additive; unknown future sample types fall back to
        // the widest current C-representable one so the ABI stays total.
        _ => ptiff_pixel_type::PTIFF_PIXEL_FLOAT64,
    }
}

/// C enum value (`i32`) -> core [`PixelType`]. Unknown values map to [`PixelType::UInt8`]
/// (the C header only ever emits 0..=4; anything else is a caller error the
/// higher layers reject, but this keeps the mapping total and non-panicking).
pub fn pixel_type_from_c(v: i32) -> PixelType {
    match v {
        x if x == ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32 => PixelType::UInt8,
        x if x == ptiff_pixel_type::PTIFF_PIXEL_UINT16 as i32 => PixelType::UInt16,
        x if x == ptiff_pixel_type::PTIFF_PIXEL_UINT32 as i32 => PixelType::UInt32,
        x if x == ptiff_pixel_type::PTIFF_PIXEL_FLOAT32 as i32 => PixelType::Float32,
        x if x == ptiff_pixel_type::PTIFF_PIXEL_FLOAT64 as i32 => PixelType::Float64,
        _ => PixelType::UInt8,
    }
}

/// Core [`CompressionKind`] -> C enum value.
pub fn compression_to_c(c: CompressionKind) -> ptiff_compression_kind {
    match c {
        CompressionKind::None => ptiff_compression_kind::PTIFF_COMPRESSION_NONE,
        CompressionKind::Lzw => ptiff_compression_kind::PTIFF_COMPRESSION_LZW,
        CompressionKind::Deflate => ptiff_compression_kind::PTIFF_COMPRESSION_DEFLATE,
        CompressionKind::Jpeg => ptiff_compression_kind::PTIFF_COMPRESSION_JPEG,
        _ => ptiff_compression_kind::PTIFF_COMPRESSION_NONE,
    }
}

/// C enum value (`i32`) -> core [`CompressionKind`]. Unknown values map to [`CompressionKind::None`].
pub fn compression_from_c(v: i32) -> CompressionKind {
    match v {
        x if x == ptiff_compression_kind::PTIFF_COMPRESSION_LZW as i32 => CompressionKind::Lzw,
        x if x == ptiff_compression_kind::PTIFF_COMPRESSION_DEFLATE as i32 => {
            CompressionKind::Deflate
        }
        x if x == ptiff_compression_kind::PTIFF_COMPRESSION_JPEG as i32 => CompressionKind::Jpeg,
        _ => CompressionKind::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_type_c_mapping_is_round_trip_stable() {
        for p in [
            PixelType::UInt8,
            PixelType::UInt16,
            PixelType::UInt32,
            PixelType::Float32,
            PixelType::Float64,
        ] {
            let c = pixel_type_to_c(p) as i32;
            assert_eq!(pixel_type_from_c(c), p);
        }
        // Header-index anchor: UInt8 is 0, Float64 is 4.
        assert_eq!(pixel_type_to_c(PixelType::UInt8) as i32, 0);
        assert_eq!(pixel_type_to_c(PixelType::Float64) as i32, 4);
    }

    #[test]
    fn compression_c_mapping_is_round_trip_stable() {
        for c in [
            CompressionKind::None,
            CompressionKind::Lzw,
            CompressionKind::Deflate,
            CompressionKind::Jpeg,
        ] {
            let cc = compression_to_c(c) as i32;
            assert_eq!(compression_from_c(cc), c);
        }
    }

    #[test]
    fn descriptor_round_trips_via_c_shape() {
        let mut d = ImageDescriptor::new(40, 30);
        d.pixel_type = PixelType::UInt16;
        d.channel_count = 3;
        d.tile_info = Some(TileInfo::new(16, 16));
        d.compression = Some(CompressionKind::Lzw);
        let c = descriptor_to_c(&d);
        let back = descriptor_from_c(&c);
        assert_eq!(back.width, 40);
        assert_eq!(back.height, 30);
        assert_eq!(back.pixel_type, PixelType::UInt16);
        assert_eq!(back.channel_count, 3);
        assert_eq!(back.tile_info, Some(TileInfo::new(16, 16)));
        assert_eq!(back.compression, Some(CompressionKind::Lzw));
    }
}
