#pragma once

#include <cstdint>

namespace ptiff::io::backend::tiff::policy {

/// Compile-time counterpart of ptiff::PixelType, kept as its own enum so the policy layer never
/// depends on the runtime image domain enum (the write path can convert later if needed).
enum class PixelValue : std::uint16_t { UInt8 = 0, UInt16 = 1, UInt32 = 2, Float32 = 3 };

/// Baseline TIFF 6.0 tag ids the policy writer emits. Mirrors the runtime TagId enum
/// (tiff_tag.hpp) 1:1, but lives in the compile-time namespace and is used exclusively in
/// constexpr contexts.
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
    Predictor = 317,
    TileWidth = 322,
    TileLength = 323,
    TileOffsets = 324,
    TileByteCounts = 325,
    SampleFormat = 339,
};

} // namespace ptiff::io::backend::tiff::policy
