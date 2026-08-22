#pragma once

#include <cstdint>
#include <map>
#include <string>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/image/pixel_type.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io::backend::tiff {

/// One strip or tile's pixel-data byte range, in row-major order (matches
/// TileLayout::columns()/rows() iteration: row-major, column fastest).
struct TileByteRange {
    std::uint64_t offset = 0;
    std::uint64_t byteCount = 0;

    friend constexpr bool operator==(const TileByteRange&, const TileByteRange&) = default;
};

/// Compression scheme this backend can decode (TIFF tag 259). Widen additively as new schemes
/// are supported -- never reuse or renumber existing values.
enum class TiffCompression { None, Lzw, PackBits, Deflate, Jpeg };

/// Predictor applied before compression (TIFF tag 317). Only horizontal differencing (2) is
/// supported; floating-point predictor (3) is rejected by interpretTiffIfd.
enum class TiffPredictor { None, HorizontalDifferencing };

/// Everything TiffBackend needs from one IFD, resolved against this backend's supported baseline
/// subset: image dimensions/pixel format, the derived TileLayout (strips modeled as one-row-tall
/// -- well, RowsPerStrip-tall -- tiles spanning the image width), and each tile/strip's byte
/// range.
struct TiffDirectory {
    std::uint32_t imageWidth = 0;
    std::uint32_t imageHeight = 0;
    ptiff::PixelType pixelType = ptiff::PixelType::UInt8;
    std::uint32_t samplesPerPixel = 1;
    io::tile::TileLayout layout;
    std::vector<TileByteRange> tileByteRanges;
    TiffCompression compression = TiffCompression::None;
    TiffPredictor predictor = TiffPredictor::None;
    Endian endian = Endian::Little;

    /// Absolute file offset of the StripByteCounts value area inside the IFD. Meaningful only
    /// when `compression != None`: the Sink patches the compressed byte count here after encode.
    std::uint64_t stripByteCountsPatchOffset = 0;

    /// libjpeg-turbo quality parameter, [0, 100]. Meaningful only when `compression == Jpeg`.
    std::uint32_t jpegQuality = 90;

    /// PTIFF extension metadata (RFC-7002) read from the private tags 65001-65005, flattened
    /// into StorageModel-style fields with the `ptiff.<domain>.<key>` convention (e.g.
    /// `ptiff.spice.frame`). Empty when the IFD carries no PTIFF extension tags. These are
    /// decoded in `interpretTiffIfd`; a private tag whose payload is not a valid PTIFF extension
    /// payload is skipped (treated as absent) so third-party private tags and forward-compatible
    /// payloads never fail the parse.
    std::map<std::string, std::string> ptiffFields;
};

/// Interprets a parsed IFD against this backend's supported baseline tag subset (see the design
/// spec's Format Coverage section). Error::InvalidArgument if a required tag is missing, or any
/// tag holds an unsupported value.
[[nodiscard]] Result<TiffDirectory> interpretTiffIfd(const TiffIfd& ifd);

/// Converts a TiffDirectory into the format-neutral StorageModel TiffBackend::deserializeModel
/// returns. Fields: "imageWidth", "imageHeight", "samplesPerPixel" (decimal strings), "pixelType"
/// (one of "UInt8"/"UInt16"/"UInt32"/"Float32").
[[nodiscard]] StorageModel toStorageModel(const TiffDirectory& directory);

} // namespace ptiff::io::backend::tiff
