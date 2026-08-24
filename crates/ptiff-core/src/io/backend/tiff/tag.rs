//! TIFF tag and field-type enums plus the raw on-disk entry record.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_tag.hpp`.

/// Bit that marks a TIFF tag as a private/extension tag: tags >= 32768
/// (0x8000) have no defined public meaning and are free for private use. PTIFF's
/// five extension tags live at 65001-65005, inside this private range and not
/// colliding with common registered extensions (e.g. GeoTIFF's 33550-34735 range
/// or GDAL's 42112-42113).
pub const K_PRIVATE_TAG_BASE: u16 = 32768;

/// Baseline TIFF 6.0 tag IDs this backend reads. Not exhaustive -- only the tags
/// needed for the supported subset (see the design spec's Format Coverage
/// section).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum TagId {
    /// Image width in pixels.
    ImageWidth = 256,
    /// Image length (height) in rows.
    ImageLength = 257,
    /// Bits per sample.
    BitsPerSample = 258,
    /// Compression scheme.
    Compression = 259,
    /// Photometric interpretation.
    PhotometricInterpretation = 262,
    /// Strip byte offsets.
    StripOffsets = 273,
    /// Samples per pixel.
    SamplesPerPixel = 277,
    /// Rows per strip.
    RowsPerStrip = 278,
    /// Strip byte counts.
    StripByteCounts = 279,
    /// Planar configuration.
    PlanarConfiguration = 284,
    /// Horizontal differencing predictor.
    Predictor = 317,
    /// Tile width.
    TileWidth = 322,
    /// Tile length (height).
    TileLength = 323,
    /// Tile byte offsets.
    TileOffsets = 324,
    /// Tile byte counts.
    TileByteCounts = 325,
    /// Extra samples.
    ExtraSamples = 338,
    /// Sample format.
    SampleFormat = 339,
    /// Chrominance subsampling (YCbCr).
    YCbCrSubSampling = 530,

    /// PTIFF extension tag: SPICE-derived geometry/pointing kernels (RFC-7002,
    /// private-tag range 65001-65005).
    PtiffSpice = 65001,
    /// PTIFF extension tag: camera model + intrinsics.
    PtiffCameraGeometry = 65002,
    /// PTIFF extension tag: planetary coordinate reference system.
    PtiffCrs = 65003,
    /// PTIFF extension tag: derived per-pixel layers (DEM, normals, ...).
    PtiffScientificLayers = 65004,
    /// PTIFF extension tag: processing history/provenance.
    PtiffProvenance = 65005,
}

impl TagId {
    /// The raw on-disk 16-bit tag id.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// TIFF 6.0 / BigTIFF field types this backend understands. All are
/// integer-valued -- no tag this backend reads uses ASCII, RATIONAL, FLOAT, or
/// DOUBLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// 8-bit unsigned integer.
    Byte,
    /// 16-bit unsigned integer.
    Short,
    /// 32-bit unsigned integer.
    Long,
    /// 64-bit unsigned integer (BigTIFF only).
    Long8,
}

impl FieldType {
    /// Parse a raw 16-bit field-type value into a [`FieldType`], or `None` for
    /// an unsupported type (the caller treats that as an InvalidArgument error
    /// or skips the entry).
    #[must_use]
    pub const fn from_u16(raw: u16) -> Option<Self> {
        match raw {
            1 => Some(FieldType::Byte),
            3 => Some(FieldType::Short),
            4 => Some(FieldType::Long),
            16 => Some(FieldType::Long8),
            _ => None,
        }
    }
}

/// Byte size of one value of `type`. Returns `None` for any field type this
/// backend does not parse.
#[must_use]
pub const fn field_type_size(field_type: FieldType) -> Option<u8> {
    match field_type {
        FieldType::Byte => Some(1),
        FieldType::Short => Some(2),
        FieldType::Long => Some(4),
        FieldType::Long8 => Some(8),
    }
}

/// One raw, unresolved IFD entry as read from the 12-byte (classic) or 20-byte
/// (BigTIFF) entry record: tag id, field type, element count, and the 4-byte
/// (classic) or 8-byte (BigTIFF) value area, which either holds the value inline
/// or an offset to it -- see `ifd::resolve_entry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTagEntry {
    /// The on-disk 16-bit tag id.
    pub tag_id: u16,
    /// The raw on-disk 16-bit field type. Unknown/unsupported values (e.g.
    /// RATIONAL=5, ASCII=2, FLOAT=11, DOUBLE=12) are preserved so the IFD parser
    /// can skip the entry rather than failing the whole parse.
    pub field_type_raw: u16,
    /// Element count (untrusted, from the file).
    pub count: u64,
    /// The 4-byte (classic) / 8-byte (BigTIFF) value area.
    pub value_area: [u8; 8],
}

/// Byte size of one value of the given raw on-disk field type. Returns `None`
/// for any field type this backend does not parse.
#[must_use]
pub const fn field_type_size_raw(field_type_raw: u16) -> Option<u8> {
    match FieldType::from_u16(field_type_raw) {
        Some(FieldType::Byte) => Some(1),
        Some(FieldType::Short) => Some(2),
        Some(FieldType::Long) => Some(4),
        Some(FieldType::Long8) => Some(8),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_ids_match_baseline_tiff() {
        assert_eq!(TagId::ImageWidth.as_u16(), 256);
        assert_eq!(TagId::TileOffsets.as_u16(), 324);
        assert_eq!(TagId::SampleFormat.as_u16(), 339);
    }

    #[test]
    fn ptiff_extension_tags_live_in_private_range() {
        for tag in [
            TagId::PtiffSpice,
            TagId::PtiffCameraGeometry,
            TagId::PtiffCrs,
            TagId::PtiffScientificLayers,
            TagId::PtiffProvenance,
        ] {
            assert!(tag.as_u16() >= K_PRIVATE_TAG_BASE, "tag above private base");
            assert!(
                (65001..=65005).contains(&tag.as_u16()),
                "extension tag in RFC-7002 range"
            );
        }
    }

    #[test]
    fn field_type_from_raw_round_trips() {
        assert_eq!(FieldType::from_u16(1), Some(FieldType::Byte));
        assert_eq!(FieldType::from_u16(3), Some(FieldType::Short));
        assert_eq!(FieldType::from_u16(4), Some(FieldType::Long));
        assert_eq!(FieldType::from_u16(16), Some(FieldType::Long8));
        // Unsupported: RATIONAL (5), ASCII (2), FLOAT (11), DOUBLE (12), ...
        assert_eq!(FieldType::from_u16(2), None);
        assert_eq!(FieldType::from_u16(5), None);
        assert_eq!(FieldType::from_u16(11), None);
        assert_eq!(FieldType::from_u16(12), None);
        assert_eq!(FieldType::from_u16(0), None);
        assert_eq!(FieldType::from_u16(999), None);
    }

    #[test]
    fn field_type_sizes() {
        assert_eq!(field_type_size(FieldType::Byte), Some(1));
        assert_eq!(field_type_size(FieldType::Short), Some(2));
        assert_eq!(field_type_size(FieldType::Long), Some(4));
        assert_eq!(field_type_size(FieldType::Long8), Some(8));
    }
}
