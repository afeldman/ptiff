#pragma once

#include <array>
#include <cstddef>
#include <cstdint>

namespace ptiff::io::backend::tiff {

/// Baseline TIFF 6.0 tag IDs this backend reads. Not exhaustive -- only the tags needed for the
/// supported subset (see the design spec's Format Coverage section).
enum class TagId : std::uint16_t {
    ImageWidth = 256,
    ImageLength = 257,
    BitsPerSample = 258,
    Compression = 259,
    PhotometricInterpretation = 262,
    StripOffsets = 273,
    SamplesPerPixel = 277,
    RowsPerStrip = 278,
    StripByteCounts = 279,
    PlanarConfiguration = 284,
    Predictor = 317,
    TileWidth = 322,
    TileLength = 323,
    TileOffsets = 324,
    TileByteCounts = 325,
    ExtraSamples = 338,
    SampleFormat = 339,
    YCbCrSubSampling = 530,

    // PTIFF extension tags (RFC-7002, private-tag range 65001-65005). Each carries a
    // versioned byte payload encoding one scientific domain; see
    // <ptiff/io/backend/tiff/ptiff_metadata.hpp>. Readers that do not know these tags ignore
    // them (TIFF 6.0 requires unknown tags be skipped), preserving interoperability with the
    // broad TIFF/BigTIFF tooling ecosystem.
    PtiffSpice = 65001,            ///< SPICE-derived geometry/pointing kernels.
    PtiffCameraGeometry = 65002,   ///< Camera model + intrinsics.
    PtiffCrs = 65003,              ///< Planetary coordinate reference system.
    PtiffScientificLayers = 65004, ///< Derived per-pixel layers (DEM, normals, ...).
    PtiffProvenance = 65005,       ///< Processing history/provenance.
};

/// TIFF 6.0 / BigTIFF field types this backend understands. All are integer-valued -- no tag
/// this backend reads uses ASCII, RATIONAL, FLOAT, or DOUBLE.
enum class FieldType : std::uint16_t {
    Byte = 1,
    Short = 3,
    Long = 4,
    Long8 = 16, // BigTIFF only
};

/// Byte size of one value of `type`. Returns 0 for any field type this backend does not parse
/// (the caller treats that as Error::InvalidArgument).
[[nodiscard]] constexpr std::uint8_t fieldTypeSize(FieldType type) noexcept {
    switch (type) {
    case FieldType::Byte:
        return 1;
    case FieldType::Short:
        return 2;
    case FieldType::Long:
        return 4;
    case FieldType::Long8:
        return 8;
    }
    return 0;
}

/// One raw, unresolved IFD entry as read from the 12-byte (classic) or 20-byte (BigTIFF) entry
/// record: tag id, field type, element count, and the 4-byte (classic) or 8-byte (BigTIFF) value
/// area, which either holds the value inline or an offset to it -- see tiff_ifd.cpp's
/// resolveEntry.
struct RawTagEntry {
    std::uint16_t tagId = 0;
    FieldType fieldType = FieldType::Byte;
    std::uint64_t count = 0;
    std::array<std::byte, 8> valueArea{};
};

} // namespace ptiff::io::backend::tiff

/// Bit that marks a TIFF tag as a private/extension tag in the TIFF 6.0 / TIFF-E
/// (TIFF-EP) scheme: tags >= 32768 (0x8000) have no defined public meaning and are free for
/// private use (TIFF 6.0 §9 note; TIFF Supplement 2 private-tag range). PTIFF's five extension
/// tags live at 65001-65005, inside this private range and not colliding with common registered
/// extensions (e.g. GeoTIFF's 33550-34735 range or GDAL's 42112-42113).
inline constexpr std::uint16_t kPrivateTagBase = 32768;
