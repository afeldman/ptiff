#include <cstddef>
#include <string>
#include <string_view>

#include <ptiff/io/backend/tiff/checked_arithmetic.hpp>
#include <ptiff/io/backend/tiff/ptiff_metadata.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>

namespace ptiff::io::backend::tiff {

namespace {

constexpr std::uint16_t tagId(TagId id) {
    return static_cast<std::uint16_t>(id);
}

// Maps each PTIFF extension tag to the StorageModel field prefix under which its decoded
// records are stored (`ptiff.<prefix>.<key>`), matching what the write path emits.
std::string_view ptiffPrefixForTag(std::uint16_t tag) noexcept {
    switch (tag) {
    case tagId(TagId::PtiffSpice):
        return "spice";
    case tagId(TagId::PtiffCameraGeometry):
        return "camera";
    case tagId(TagId::PtiffCrs):
        return "crs";
    case tagId(TagId::PtiffScientificLayers):
        return "layers";
    case tagId(TagId::PtiffProvenance):
        return "provenance";
    default:
        return "";
    }
}

// Decodes the value archive of one PTIFF extension tag into `directory.ptiffFields` under the
// `ptiff.<prefix>.<key>` convention. A payload that is absent, or does not decode as a PTIFF
// extension payload, is skipped -- third-party private byte blobs and forward-compatible
// future versions must not fail the whole directory parse.
void decodePtiffTag(const TiffIfd& ifd, std::uint16_t tag, TiffDirectory& directory) {
    const auto values = ifd.tag(tag);
    if (!values.has_value()) {
        return;
    }
    const auto records = decodeMetadataPayload(*values);
    if (!records.has_value()) {
        return;
    }
    const auto prefix = ptiffPrefixForTag(tag);
    if (prefix.empty()) {
        return;
    }
    const std::string pfx = "ptiff." + std::string(prefix) + ".";
    for (const auto& rec : *records) {
        directory.ptiffFields[pfx + rec.key] = rec.value;
    }
}

std::string_view compressionFieldValue(TiffCompression compression) noexcept {
    switch (compression) {
    case TiffCompression::None: {
        return "None";
    }
    case TiffCompression::Lzw: {
        return "Lzw";
    }
    case TiffCompression::PackBits: {
        return "PackBits";
    }
    case TiffCompression::Deflate: {
        return "Deflate";
    }
    case TiffCompression::Jpeg: {
        return "Jpeg";
    }
    }
    return "None";
}

std::string_view predictorFieldValue(TiffPredictor predictor) noexcept {
    switch (predictor) {
    case TiffPredictor::None: {
        return "None";
    }
    case TiffPredictor::HorizontalDifferencing: {
        return "HorizontalDifferencing";
    }
    }
    return "None";
}

/// A required tag that is absent is reported as Error::InvalidArgument (NotFound is reserved for
/// genuinely optional lookups), matching interpretTiffIfd's documented contract.
Result<std::uint64_t> requireSingleValue(const TiffIfd& ifd, TagId id) {
    auto value = ifd.singleValue(tagId(id));
    if (!value.has_value()) {
        if (value.error().code() == ErrorCode::NotFound) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "interpretTiffIfd: required tag is missing"});
        }
        return std::unexpected(value.error());
    }
    return value;
}

Result<std::vector<std::uint64_t>> requireTag(const TiffIfd& ifd, TagId id) {
    auto values = ifd.tag(tagId(id));
    if (!values.has_value()) {
        if (values.error().code() == ErrorCode::NotFound) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "interpretTiffIfd: required tag is missing"});
        }
        return std::unexpected(values.error());
    }
    return values;
}

/// Builds one TileByteRange per strip/tile. offsets and byteCounts must have matched lengths --
/// a malformed TIFF with mismatched StripOffsets/StripByteCounts (or Tile equivalents) arrays is
/// rejected rather than silently truncated to the shorter array. A claim of more entries than a
/// single tag array can legally hold (RFC-0001 §13 resource guard) is rejected up front.
Result<std::vector<TileByteRange>> buildByteRanges(const std::vector<std::uint64_t>& offsets,
                                                   const std::vector<std::uint64_t>& byteCounts) {
    if (offsets.size() != byteCounts.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "interpretTiffIfd: offsets and byteCounts arrays have mismatched lengths"});
    }
    if (offsets.size() > kMaxTagCount) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "interpretTiffIfd: strip/tile table exceeds a safe element bound"});
    }

    std::vector<TileByteRange> ranges;
    ranges.reserve(offsets.size());
    for (std::size_t i = 0; i < offsets.size(); ++i) {
        ranges.push_back(TileByteRange{.offset = offsets[i], .byteCount = byteCounts[i]});
    }
    return ranges;
}

/// SamplesPerPixel, defaulting to 1. Grayscale (1), RGB (3), and arbitrary multispectral band
/// counts (2..512) are supported.
Result<std::uint64_t> resolveSamplesPerPixel(const TiffIfd& ifd) {
    auto samplesPerPixel = ifd.singleValueOr(tagId(TagId::SamplesPerPixel), 1);
    if (!samplesPerPixel.has_value()) {
        return std::unexpected(samplesPerPixel.error());
    }
    if (*samplesPerPixel < 1 || *samplesPerPixel > 512) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "interpretTiffIfd: unsupported SamplesPerPixel"});
    }
    return samplesPerPixel;
}

/// Compression, defaulting to 1 (uncompressed). Supported: 1 (None), 5 (LZW), 32773 (PackBits),
/// 8 (Deflate, the modern/Adobe tag value) and 32946 (the legacy "old-style Deflate" tag value --
/// both resolve to the same zlib-wrapped stream format and the same TiffCompression::Deflate),
/// and 7 (new-style JPEG, TiffCompression::Jpeg).
Result<TiffCompression> resolveCompression(const TiffIfd& ifd) {
    auto compression = ifd.singleValueOr(tagId(TagId::Compression), 1);
    if (!compression.has_value()) {
        return std::unexpected(compression.error());
    }
    switch (*compression) {
    case 1: {
        return TiffCompression::None;
    }
    case 5: {
        return TiffCompression::Lzw;
    }
    case 32773: {
        return TiffCompression::PackBits;
    }
    case 8:
    case 32946: {
        return TiffCompression::Deflate;
    }
    case 7: {
        return TiffCompression::Jpeg;
    }
    default: {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "interpretTiffIfd: unsupported Compression value " +
                                         std::to_string(*compression)});
    }
    }
}

/// Predictor, defaulting to 1 (none). Supported: 1 (None), 2 (horizontal differencing). 2 is
/// rejected when paired with a float32 pixelType or Jpeg compression -- both undefined by the
/// TIFF spec.
Result<TiffPredictor>
resolvePredictor(const TiffIfd& ifd, ptiff::PixelType pixelType, TiffCompression compression) {
    auto predictorValue = ifd.singleValueOr(tagId(TagId::Predictor), 1);
    if (!predictorValue.has_value()) {
        return std::unexpected(predictorValue.error());
    }
    if (*predictorValue == 1) {
        return TiffPredictor::None;
    }
    if (*predictorValue == 2) {
        if (pixelType == ptiff::PixelType::Float32) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "interpretTiffIfd: Predictor=2 is not defined for float32 samples"});
        }
        if (compression == TiffCompression::Jpeg) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "interpretTiffIfd: Predictor=2 is not defined for Jpeg compression"});
        }
        return TiffPredictor::HorizontalDifferencing;
    }
    return std::unexpected(
        Error{ErrorCode::InvalidArgument,
              "interpretTiffIfd: unsupported Predictor value " + std::to_string(*predictorValue)});
}

/// PhotometricInterpretation, required. WhiteIsZero (0), BlackIsZero (1), and RGB (2) are
/// supported unconditionally; YCbCr (6) is accepted here and cross-validated against compression
/// and samplesPerPixel afterwards in interpretTiffIfd (it is only valid for Jpeg + RGB).
Result<std::uint64_t> resolvePhotometricInterpretation(const TiffIfd& ifd) {
    auto photometric = requireSingleValue(ifd, TagId::PhotometricInterpretation);
    if (!photometric.has_value()) {
        return std::unexpected(photometric.error());
    }
    if (*photometric != 0 && *photometric != 1 && *photometric != 2 && *photometric != 6) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "interpretTiffIfd: unsupported PhotometricInterpretation"});
    }
    return photometric;
}

/// PlanarConfiguration, defaulting to 1 (chunky). Planar (2) storage is not supported.
Result<std::uint64_t> resolvePlanarConfiguration(const TiffIfd& ifd) {
    auto planarConfig = ifd.singleValueOr(tagId(TagId::PlanarConfiguration), 1);
    if (!planarConfig.has_value()) {
        return std::unexpected(planarConfig.error());
    }
    if (*planarConfig != 1) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "interpretTiffIfd: unsupported PlanarConfiguration"});
    }
    return planarConfig;
}

/// The strip/tile grid dimensions and which tags hold the byte offsets/counts, resolved
/// according to whichever of the two mutually-exclusive layouts (hasStrips) this IFD uses.
struct StripOrTileLayout {
    std::uint32_t tileWidth = 0;
    std::uint32_t tileHeight = 0;
    std::uint16_t offsetsTag = 0;
    std::uint16_t byteCountsTag = 0;
};

Result<StripOrTileLayout>
resolveStripOrTileLayout(const TiffIfd& ifd, bool hasStrips, std::uint32_t imageWidth) {
    if (hasStrips) {
        auto rowsPerStrip = requireSingleValue(ifd, TagId::RowsPerStrip);
        if (!rowsPerStrip.has_value()) {
            return std::unexpected(rowsPerStrip.error());
        }
        return StripOrTileLayout{.tileWidth = imageWidth,
                                 .tileHeight = static_cast<std::uint32_t>(*rowsPerStrip),
                                 .offsetsTag = tagId(TagId::StripOffsets),
                                 .byteCountsTag = tagId(TagId::StripByteCounts)};
    }

    auto tileWidthValue = requireSingleValue(ifd, TagId::TileWidth);
    if (!tileWidthValue.has_value()) {
        return std::unexpected(tileWidthValue.error());
    }
    auto tileLengthValue = requireSingleValue(ifd, TagId::TileLength);
    if (!tileLengthValue.has_value()) {
        return std::unexpected(tileLengthValue.error());
    }
    return StripOrTileLayout{.tileWidth = static_cast<std::uint32_t>(*tileWidthValue),
                             .tileHeight = static_cast<std::uint32_t>(*tileLengthValue),
                             .offsetsTag = tagId(TagId::TileOffsets),
                             .byteCountsTag = tagId(TagId::TileByteCounts)};
}

} // namespace

Result<TiffDirectory> interpretTiffIfd(const TiffIfd& ifd) {
    auto width = requireSingleValue(ifd, TagId::ImageWidth);
    if (!width.has_value()) {
        return std::unexpected(width.error());
    }
    auto height = requireSingleValue(ifd, TagId::ImageLength);
    if (!height.has_value()) {
        return std::unexpected(height.error());
    }

    auto samplesPerPixel = resolveSamplesPerPixel(ifd);
    if (!samplesPerPixel.has_value()) {
        return std::unexpected(samplesPerPixel.error());
    }

    auto bitsPerSampleValues = requireTag(ifd, TagId::BitsPerSample);
    if (!bitsPerSampleValues.has_value()) {
        return std::unexpected(bitsPerSampleValues.error());
    }
    auto uniform = requireUniformBitsPerSample(*bitsPerSampleValues);
    if (!uniform.has_value()) {
        return std::unexpected(uniform.error());
    }

    auto compression = resolveCompression(ifd);
    if (!compression.has_value()) {
        return std::unexpected(compression.error());
    }

    auto photometric = resolvePhotometricInterpretation(ifd);
    if (!photometric.has_value()) {
        return std::unexpected(photometric.error());
    }

    auto planarConfig = resolvePlanarConfiguration(ifd);
    if (!planarConfig.has_value()) {
        return std::unexpected(planarConfig.error());
    }

    auto sampleFormat = ifd.singleValueOr(tagId(TagId::SampleFormat), 1);
    if (!sampleFormat.has_value()) {
        return std::unexpected(sampleFormat.error());
    }
    auto pixelType = resolvePixelType(bitsPerSampleValues->front(), *sampleFormat);
    if (!pixelType.has_value()) {
        return std::unexpected(pixelType.error());
    }

    auto predictor = resolvePredictor(ifd, *pixelType, *compression);
    if (!predictor.has_value()) {
        return std::unexpected(predictor.error());
    }

    if (*compression == TiffCompression::Jpeg && *pixelType != ptiff::PixelType::UInt8) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "interpretTiffIfd: Jpeg compression requires UInt8 samples"});
    }
    if (*photometric == 6 && *compression != TiffCompression::Jpeg) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "interpretTiffIfd: PhotometricInterpretation=YCbCr(6) requires Jpeg "
                  "compression"});
    }
    if (*compression == TiffCompression::Jpeg && *samplesPerPixel == 1 && *photometric != 0 &&
        *photometric != 1) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "interpretTiffIfd: Jpeg grayscale requires PhotometricInterpretation "
                  "WhiteIsZero(0) or BlackIsZero(1)"});
    }
    if (*compression == TiffCompression::Jpeg && *samplesPerPixel == 3 && *photometric != 6) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "interpretTiffIfd: Jpeg RGB requires PhotometricInterpretation YCbCr(6)"});
    }

    const bool hasStrips = ifd.tag(tagId(TagId::StripOffsets)).has_value();
    const bool hasTiles = ifd.tag(tagId(TagId::TileOffsets)).has_value();
    if (hasStrips == hasTiles) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "interpretTiffIfd: image must be exactly one of stripped "
                                     "or tiled"});
    }

    TiffDirectory directory;
    directory.imageWidth = static_cast<std::uint32_t>(*width);
    directory.imageHeight = static_cast<std::uint32_t>(*height);
    directory.pixelType = *pixelType;
    directory.samplesPerPixel = static_cast<std::uint32_t>(*samplesPerPixel);

    auto stripOrTile = resolveStripOrTileLayout(ifd, hasStrips, directory.imageWidth);
    if (!stripOrTile.has_value()) {
        return std::unexpected(stripOrTile.error());
    }

    auto offsets = requireTag(ifd, static_cast<TagId>(stripOrTile->offsetsTag));
    if (!offsets.has_value()) {
        return std::unexpected(offsets.error());
    }
    auto byteCounts = requireTag(ifd, static_cast<TagId>(stripOrTile->byteCountsTag));
    if (!byteCounts.has_value()) {
        return std::unexpected(byteCounts.error());
    }

    auto byteRanges = buildByteRanges(*offsets, *byteCounts);
    if (!byteRanges.has_value()) {
        return std::unexpected(byteRanges.error());
    }

    directory.layout = io::tile::TileLayout{
        .tileSize = {.width = stripOrTile->tileWidth, .height = stripOrTile->tileHeight},
        .imageWidth = directory.imageWidth,
        .imageHeight = directory.imageHeight,
        .levelCount = 1};
    directory.tileByteRanges = std::move(*byteRanges);
    directory.compression = *compression;
    directory.predictor = *predictor;

    // Surface any PTIFF extension tags into the directory's flat metadata map. Each is decoded
    // defensively; a malformed/unknown payload simply leaves that domain absent.
    decodePtiffTag(ifd, tagId(TagId::PtiffSpice), directory);
    decodePtiffTag(ifd, tagId(TagId::PtiffCameraGeometry), directory);
    decodePtiffTag(ifd, tagId(TagId::PtiffCrs), directory);
    decodePtiffTag(ifd, tagId(TagId::PtiffScientificLayers), directory);
    decodePtiffTag(ifd, tagId(TagId::PtiffProvenance), directory);

    return directory;
}

StorageModel toStorageModel(const TiffDirectory& directory) {
    StorageModel model;
    model.setField("imageWidth", std::to_string(directory.imageWidth));
    model.setField("imageHeight", std::to_string(directory.imageHeight));
    model.setField("samplesPerPixel", std::to_string(directory.samplesPerPixel));
    model.setField("pixelType", std::string{pixelTypeFieldValue(directory.pixelType)});
    model.setField("compression", std::string{compressionFieldValue(directory.compression)});
    model.setField("predictor", std::string{predictorFieldValue(directory.predictor)});
    model.setField("jpegQuality", std::to_string(directory.jpegQuality));
    for (const auto& [key, value] : directory.ptiffFields) {
        model.setField(key, value);
    }
    return model;
}

} // namespace ptiff::io::backend::tiff
