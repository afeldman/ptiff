#include <algorithm>
#include <charconv>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include <ptiff/io/backend/tiff/ptiff_metadata.hpp>
#include <ptiff/io/backend/tiff/tiff_directory_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>

namespace ptiff::io::backend::tiff {

namespace {

constexpr std::uint16_t tagId(TagId id) {
    return static_cast<std::uint16_t>(id);
}

Result<std::uint32_t> requireUint32Field(const StorageModel& model, std::string_view key) {
    auto value = model.field(key);
    if (!value.has_value()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "planTiffWrite: missing required field \"" + std::string{key} + "\""});
    }
    std::uint32_t parsed = 0;
    auto parseResult = std::from_chars(value->data(), value->data() + value->size(), parsed);
    if (parseResult.ec != std::errc{} || parseResult.ptr != value->data() + value->size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: field \"" + std::string{key} +
                                         "\" is not a valid unsigned integer"});
    }
    return parsed;
}

Result<std::optional<TiffCompression>> parseCompression(const StorageModel& model) {
    auto value = model.field("compression");
    if (!value.has_value())
        return std::nullopt;
    if (*value == "None")
        return TiffCompression::None;
    if (*value == "PackBits")
        return TiffCompression::PackBits;
    if (*value == "LZW")
        return TiffCompression::Lzw;
    if (*value == "Deflate")
        return TiffCompression::Deflate;
    if (*value == "Jpeg")
        return TiffCompression::Jpeg;
    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                 "planTiffWrite: unsupported compression \"" + *value + "\""});
}

Result<std::optional<TiffPredictor>> parsePredictor(const StorageModel& model) {
    auto value = model.field("predictor");
    if (!value.has_value())
        return std::nullopt;
    if (*value == "None")
        return TiffPredictor::None;
    if (*value == "HorizontalDifferencing")
        return TiffPredictor::HorizontalDifferencing;
    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                 "planTiffWrite: unsupported predictor \"" + *value + "\""});
}

/// "jpegQuality" field, defaulting to 90 when absent. Error::InvalidArgument if present but not a
/// valid unsigned integer, or outside [0, 100].
Result<std::uint32_t> parseJpegQuality(const StorageModel& model) {
    auto value = model.field("jpegQuality");
    if (!value.has_value()) {
        return 90U;
    }
    std::uint32_t parsed = 0;
    auto parseResult = std::from_chars(value->data(), value->data() + value->size(), parsed);
    if (parseResult.ec != std::errc{} || parseResult.ptr != value->data() + value->size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "planTiffWrite: field \"jpegQuality\" is not a valid unsigned integer"});
    }
    if (parsed > 100) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "planTiffWrite: jpegQuality must be in [0, 100]"});
    }
    return parsed;
}

// Appends PTIFF extension tag entries (65001-65005) to `entries` for every domain whose
// fields are present in `model` under the `ptiff.<domain>.*` convention. Each present domain is
// serialized into a single BYTE-typed private tag carrying the versioned metadata payload. A
// model with no ptiff.* fields emits nothing, so a plain TIFF writer/reader is unaffected.
// Error::InvalidArgument if a present domain's payload cannot be encoded (e.g. a key too long).
Result<void> appendPtiffTags(const StorageModel& model, std::vector<TiffIfdEntryToWrite>& entries) {
    struct Dom {
        TagId tag;
        const char* prefix;
    };
    constexpr Dom domains[] = {
        {TagId::PtiffSpice, "spice"},
        {TagId::PtiffCameraGeometry, "camera"},
        {TagId::PtiffCrs, "crs"},
        {TagId::PtiffScientificLayers, "layers"},
        {TagId::PtiffProvenance, "provenance"},
    };
    for (const auto& dom : domains) {
        auto records = recordsFromStorageModel(model, dom.prefix);
        if (records.empty()) {
            continue;
        }
        auto payload = encodeMetadataPayload(records);
        if (!payload.has_value()) {
            return std::unexpected(payload.error());
        }
        std::vector<std::uint32_t> byteValues;
        byteValues.reserve(payload->size());
        for (const auto b : *payload) {
            byteValues.push_back(static_cast<std::uint32_t>(static_cast<unsigned char>(b)));
        }
        entries.push_back(TiffIfdEntryToWrite{
            .tagId = static_cast<std::uint16_t>(dom.tag),
            .fieldType = FieldType::Byte,
            .values = std::move(byteValues),
        });
    }
    return {};
}

Result<bool> parseContainer(const StorageModel& model) {
    auto value = model.field("container");
    if (!value.has_value() || *value == "Classic")
        return false;
    if (*value == "BigTiff")
        return true;
    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                 "planTiffWrite: unsupported container \"" + *value + "\""});
}

/// Multiplies `a` and `b`, rejecting the result rather than silently overflowing.
Result<std::uint64_t> checkedMultiply(std::uint64_t a, std::uint64_t b) {
    if (a != 0 && b > std::numeric_limits<std::uint64_t>::max() / a) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "planTiffWrite: strip size computation overflows"});
    }
    return a * b;
}

/// Parses the optional "tileWidth"/"tileHeight" field pair. std::nullopt when both are absent
/// (strip layout). Error::InvalidArgument when only one is set, either is unparsable, or either
/// is zero or not a multiple of 16 (TIFF 6.0 baseline requirement for tiled images).
Result<std::optional<std::pair<std::uint32_t, std::uint32_t>>>
parseTileSize(const StorageModel& model) {
    const bool hasWidth = model.field("tileWidth").has_value();
    const bool hasHeight = model.field("tileHeight").has_value();
    if (!hasWidth && !hasHeight) {
        return std::nullopt;
    }
    if (hasWidth != hasHeight) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: \"tileWidth\" and \"tileHeight\" must both "
                                     "be set or both be absent"});
    }
    auto width = requireUint32Field(model, "tileWidth");
    if (!width.has_value()) {
        return std::unexpected(width.error());
    }
    auto height = requireUint32Field(model, "tileHeight");
    if (!height.has_value()) {
        return std::unexpected(height.error());
    }
    if (*width == 0 || *height == 0 || *width % 16 != 0 || *height % 16 != 0) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: tileWidth/tileHeight must be nonzero "
                                     "multiples of 16"});
    }
    return std::make_pair(*width, *height);
}

} // namespace

Result<TiffWritePlan> planTiffWrite(const StorageModel& model) {
    auto imageWidth = requireUint32Field(model, "imageWidth");
    if (!imageWidth.has_value()) {
        return std::unexpected(imageWidth.error());
    }
    auto imageHeight = requireUint32Field(model, "imageHeight");
    if (!imageHeight.has_value()) {
        return std::unexpected(imageHeight.error());
    }
    auto samplesPerPixel = requireUint32Field(model, "samplesPerPixel");
    if (!samplesPerPixel.has_value()) {
        return std::unexpected(samplesPerPixel.error());
    }
    if (*samplesPerPixel < 1 || *samplesPerPixel > 512) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "planTiffWrite: unsupported samplesPerPixel"});
    }

    auto pixelTypeField = model.field("pixelType");
    if (!pixelTypeField.has_value()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: missing required field \"pixelType\""});
    }
    auto pixelType = pixelTypeFromFieldValue(*pixelTypeField);
    if (!pixelType.has_value()) {
        return std::unexpected(pixelType.error());
    }

    auto tileSizeResult = parseTileSize(model);
    if (!tileSizeResult.has_value()) {
        return std::unexpected(tileSizeResult.error());
    }
    const auto& tileSizeOpt = *tileSizeResult;
    const bool tiled = tileSizeOpt.has_value();
    std::uint32_t tileWidthValue = 0;
    std::uint32_t tileHeightValue = 0;
    if (tiled) {
        std::tie(tileWidthValue, tileHeightValue) = *tileSizeOpt;
    }

    auto compression = parseCompression(model);
    if (!compression.has_value())
        return std::unexpected(compression.error());
    auto predictor = parsePredictor(model);
    if (!predictor.has_value())
        return std::unexpected(predictor.error());
    const auto comp = compression->value_or(TiffCompression::None);
    const auto pred = predictor->value_or(TiffPredictor::None);

    if (tiled && comp != TiffCompression::None) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: tiled write does not support compression "
                                     "yet"});
    }
    if (tiled && pred != TiffPredictor::None) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: tiled write does not support a predictor "
                                     "yet"});
    }
    if (comp == TiffCompression::Jpeg && *pixelType != ptiff::PixelType::UInt8) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: Jpeg compression requires UInt8 pixelType"});
    }

    auto jpegQuality = parseJpegQuality(model);
    if (!jpegQuality.has_value())
        return std::unexpected(jpegQuality.error());

    auto container = parseContainer(model);
    if (!container.has_value())
        return std::unexpected(container.error());
    const bool isBigTiff = *container;

    // Horizontal differencing is defined only for integer samples (UInt8/UInt16/UInt32).
    // Check the pixel TYPE directly, not bytesPerSample: Float32 also has 4 bytes/sample, so a
    // bytesPerSample-based check would wrongly admit floating-point samples.
    if (pred == TiffPredictor::HorizontalDifferencing && *pixelType != ptiff::PixelType::UInt8 &&
        *pixelType != ptiff::PixelType::UInt16 && *pixelType != ptiff::PixelType::UInt32) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWrite: horizontal differencing requires integer "
                                     "samples (UInt8/UInt16/UInt32)"});
    }
    if (pred == TiffPredictor::HorizontalDifferencing && comp == TiffCompression::Jpeg) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "planTiffWrite: Predictor is not defined for Jpeg compression"});
    }

    const std::uint32_t photometric = comp == TiffCompression::Jpeg && *samplesPerPixel == 3
                                          ? 6U                      // YCbCr
                                      : *samplesPerPixel == 3 ? 2U  // RGB
                                                              : 1U; // BlackIsZero (gray or N-band)
    const std::uint32_t compressionTag = comp == TiffCompression::Lzw        ? 5U
                                         : comp == TiffCompression::PackBits ? 32773U
                                         : comp == TiffCompression::Deflate  ? 8U
                                         : comp == TiffCompression::Jpeg     ? 7U
                                                                             : 1U;

    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = tagId(TagId::ImageWidth), .fieldType = FieldType::Long, .values = {*imageWidth}},
        {.tagId = tagId(TagId::ImageLength),
         .fieldType = FieldType::Long,
         .values = {*imageHeight}},
        {.tagId = tagId(TagId::BitsPerSample),
         .fieldType = FieldType::Short,
         .values = std::vector<std::uint32_t>(*samplesPerPixel, bitsPerSampleFor(*pixelType))},
        {.tagId = tagId(TagId::Compression),
         .fieldType = FieldType::Long,
         .values = {compressionTag}},
        {.tagId = tagId(TagId::PhotometricInterpretation),
         .fieldType = FieldType::Long,
         .values = {photometric}},
        {.tagId = tagId(TagId::SamplesPerPixel),
         .fieldType = FieldType::Long,
         .values = {*samplesPerPixel}},
        {.tagId = tagId(TagId::SampleFormat),
         .fieldType = FieldType::Short,
         .values = {sampleFormatFor(*pixelType)}},
    };

    {
        const std::uint32_t baselineSamples = (photometric == 2U) ? 3U : 1U;
        if (*samplesPerPixel > baselineSamples) {
            entries.push_back(
                {.tagId = tagId(TagId::ExtraSamples),
                 .fieldType = FieldType::Short,
                 .values = std::vector<std::uint32_t>(*samplesPerPixel - baselineSamples, 0U)});
        }
    }

    std::uint32_t tileColumns = 0;
    std::uint32_t tileRows = 0;
    std::uint64_t tileByteSize = 0;
    std::uint64_t stripByteCountValue = 0;

    if (tiled) {
        tileColumns = (*imageWidth + tileWidthValue - 1) / tileWidthValue;
        tileRows = (*imageHeight + tileHeightValue - 1) / tileHeightValue;
        const auto tileCount = static_cast<std::uint64_t>(tileColumns) * tileRows;

        auto tileRowBytesStep = checkedMultiply(tileWidthValue, *samplesPerPixel);
        if (!tileRowBytesStep.has_value()) {
            return std::unexpected(tileRowBytesStep.error());
        }
        auto tileRowBytes = checkedMultiply(*tileRowBytesStep, bytesPerSample(*pixelType));
        if (!tileRowBytes.has_value()) {
            return std::unexpected(tileRowBytes.error());
        }
        auto tileByteSizeResult = checkedMultiply(*tileRowBytes, tileHeightValue);
        if (!tileByteSizeResult.has_value()) {
            return std::unexpected(tileByteSizeResult.error());
        }
        tileByteSize = *tileByteSizeResult;

        entries.push_back({.tagId = tagId(TagId::TileWidth),
                           .fieldType = FieldType::Long,
                           .values = {tileWidthValue}});
        entries.push_back({.tagId = tagId(TagId::TileLength),
                           .fieldType = FieldType::Long,
                           .values = {tileHeightValue}});
        entries.push_back({.tagId = tagId(TagId::TileOffsets),
                           .fieldType = FieldType::Long,
                           .values = std::vector<std::uint32_t>(tileCount, 0)}); // patched below
        entries.push_back({.tagId = tagId(TagId::TileByteCounts),
                           .fieldType = FieldType::Long,
                           .values = std::vector<std::uint32_t>(
                               tileCount, static_cast<std::uint32_t>(tileByteSize))});
    } else {
        auto rowBytesStep = checkedMultiply(*imageWidth, *samplesPerPixel);
        if (!rowBytesStep.has_value()) {
            return std::unexpected(rowBytesStep.error());
        }
        auto rowBytes = checkedMultiply(*rowBytesStep, bytesPerSample(*pixelType));
        if (!rowBytes.has_value()) {
            return std::unexpected(rowBytes.error());
        }
        auto stripByteCount = checkedMultiply(*rowBytes, *imageHeight);
        if (!stripByteCount.has_value()) {
            return std::unexpected(stripByteCount.error());
        }
        stripByteCountValue = *stripByteCount;

        entries.push_back({.tagId = tagId(TagId::StripOffsets),
                           .fieldType = FieldType::Long,
                           .values = {0}}); // patched below
        entries.push_back({.tagId = tagId(TagId::RowsPerStrip),
                           .fieldType = FieldType::Long,
                           .values = {*imageHeight}});
        entries.push_back(
            {.tagId = tagId(TagId::StripByteCounts),
             .fieldType = FieldType::Long,
             .values = {comp == TiffCompression::None
                            ? static_cast<std::uint32_t>(stripByteCountValue)
                            : 0U}}); // 0 placeholder, back-patched by the Sink after encode
    }

    if (pred != TiffPredictor::None) {
        entries.push_back({.tagId = tagId(TagId::Predictor),
                           .fieldType = FieldType::Long,
                           .values = {2U}}); // HorizontalDifferencing
    }
    if (photometric == 6U) {
        entries.push_back({.tagId = tagId(TagId::YCbCrSubSampling),
                           .fieldType = FieldType::Short,
                           .values = {1U, 1U}}); // 4:4:4, no chroma subsampling
    }

    // Emit any PTIFF extension metadata present in the model as private TIFF tags 65001-65005.
    // This must happen before `dataOffset` is computed below so the (possibly out-of-line)
    // private-tag value bytes are included in the IFD byte size and the pixel data does not
    // overlap them.
    if (auto ptiffResult = appendPtiffTags(model, entries); !ptiffResult.has_value()) {
        return std::unexpected(ptiffResult.error());
    }

    // Compute the first strip/tile's data offset after every entry (including any Predictor
    // entry) is appended so pixel data does not overlap the final IFD.
    const std::uint64_t headerSize = isBigTiff ? kBigTiffHeaderSize : kClassicTiffHeaderSize;
    const std::uint64_t dataOffset = headerSize + tiffIfdByteSize(entries, isBigTiff);

    std::vector<TileByteRange> tileByteRanges;
    if (tiled) {
        const auto tileCount = static_cast<std::uint64_t>(tileColumns) * tileRows;
        std::vector<std::uint32_t> tileOffsets(tileCount);
        for (std::uint64_t i = 0; i < tileCount; ++i) {
            const std::uint64_t offset = dataOffset + i * tileByteSize;
            tileOffsets[i] = static_cast<std::uint32_t>(offset);
            tileByteRanges.push_back(TileByteRange{.offset = offset, .byteCount = tileByteSize});
        }
        for (auto& entry : entries) {
            if (entry.tagId == tagId(TagId::TileOffsets)) {
                entry.values = tileOffsets;
            }
        }
    } else {
        for (auto& entry : entries) {
            if (entry.tagId == tagId(TagId::StripOffsets)) {
                entry.values = {static_cast<std::uint32_t>(dataOffset)};
            }
        }
        tileByteRanges = {TileByteRange{.offset = dataOffset, .byteCount = stripByteCountValue}};
    }

    std::uint64_t byteCountPatchOffset = 0;
    if (!tiled) {
        std::vector<std::uint16_t> sortedTags;
        for (const auto& e : entries)
            sortedTags.push_back(e.tagId);
        std::sort(sortedTags.begin(), sortedTags.end());
        std::size_t byteCountIndex = 0;
        for (std::size_t i = 0; i < sortedTags.size(); ++i) {
            if (sortedTags[i] == tagId(TagId::StripByteCounts)) {
                byteCountIndex = i;
                break;
            }
        }
        // Deterministic offset of the StripByteCounts value area within its entry record:
        // tagId(2) + fieldType(2) + count field (classic 4 / BigTIFF 8) bytes precede the value
        // area.
        const std::uint64_t entryRecordSize = isBigTiff ? 20 : 12;
        const std::uint64_t entryCountFieldSize = isBigTiff ? 8 : 2;
        const std::uint64_t valueAreaOffsetWithinRecord = isBigTiff ? 12 : 8;
        byteCountPatchOffset = headerSize + entryCountFieldSize + byteCountIndex * entryRecordSize +
                               valueAreaOffsetWithinRecord;
    }

    TiffDirectory directory;
    directory.imageWidth = *imageWidth;
    directory.imageHeight = *imageHeight;
    directory.pixelType = *pixelType;
    directory.samplesPerPixel = *samplesPerPixel;
    directory.layout =
        tiled
            ? io::tile::TileLayout{.tileSize = {.width = tileWidthValue, .height = tileHeightValue},
                                   .imageWidth = *imageWidth,
                                   .imageHeight = *imageHeight,
                                   .levelCount = 1}
            : io::tile::TileLayout{.tileSize = {.width = *imageWidth, .height = *imageHeight},
                                   .imageWidth = *imageWidth,
                                   .imageHeight = *imageHeight,
                                   .levelCount = 1};
    directory.tileByteRanges = std::move(tileByteRanges);
    directory.compression = comp;
    directory.predictor = pred;
    directory.stripByteCountsPatchOffset = byteCountPatchOffset;
    directory.endian = Endian::Little;
    directory.jpegQuality = *jpegQuality;

    return TiffWritePlan{
        .directory = std::move(directory), .entries = std::move(entries), .isBigTiff = isBigTiff};
}

Result<TiffFileWritePlan> planTiffWriteMulti(std::span<const StorageModel> models) {
    if (models.empty()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "planTiffWriteMulti: at least one model is required"});
    }

    std::vector<TiffWritePlan> images;
    images.reserve(models.size());
    bool isBigTiff = false;
    for (std::size_t i = 0; i < models.size(); ++i) {
        auto plan = planTiffWrite(models[i]);
        if (!plan.has_value()) {
            return std::unexpected(plan.error());
        }
        if (i == 0) {
            isBigTiff = plan->isBigTiff;
        } else if (plan->isBigTiff != isBigTiff) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "planTiffWriteMulti: all images must agree on a container kind "
                      "(classic or BigTIFF)"});
        }
        images.push_back(std::move(*plan));
    }

    const std::uint64_t headerSize = isBigTiff ? kBigTiffHeaderSize : kClassicTiffHeaderSize;

    // Every IFD's absolute file offset, chained via NextIFD. Laid out contiguously right after
    // the header.
    std::vector<std::uint64_t> ifdOffsets(images.size());
    std::uint64_t cursor = headerSize;
    for (std::size_t i = 0; i < images.size(); ++i) {
        ifdOffsets[i] = cursor;
        const auto size = tiffIfdByteSize(images[i].entries, isBigTiff);
        if (size > std::numeric_limits<std::uint64_t>::max() - cursor) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "planTiffWriteMulti: IFD layout overflows"});
        }
        cursor += size;
    }
    // `cursor` is now the absolute offset of the first data byte.

    // Each image's data region start and reserved span. The span is the sum of its
    // tileByteRanges' byteCounts (its uncompressed payload size); for an uncompressed image this
    // is the exact on-disk size, for a compressed image it reserves enough room for the encoded
    // strips (see the write path).
    std::vector<std::uint64_t> dataOffsets(images.size());
    std::uint64_t dataCursor = cursor;
    for (std::size_t i = 0; i < images.size(); ++i) {
        dataOffsets[i] = dataCursor;
        std::uint64_t span = 0;
        for (const auto& range : images[i].directory.tileByteRanges) {
            if (range.byteCount > std::numeric_limits<std::uint64_t>::max() - span) {
                return std::unexpected(Error{ErrorCode::InvalidArgument,
                                             "planTiffWriteMulti: image data region overflows"});
            }
            span += range.byteCount;
        }
        if (span > std::numeric_limits<std::uint64_t>::max() - dataCursor) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "planTiffWriteMulti: file data layout overflows"});
        }
        dataCursor += span;
    }

    // Rebase each image's directory AND its IFD tag values to hold absolute file offsets. The
    // per-image planner sized IFD and data as if the file held only that image (IFD at
    // headerSize, data right after); correct both to the multi-IFD layout.
    for (std::size_t i = 0; i < images.size(); ++i) {
        auto& directory = images[i].directory;
        const std::uint64_t originalDataBase =
            headerSize + tiffIfdByteSize(images[i].entries, isBigTiff);
        const std::uint64_t dataDelta = dataOffsets[i] - originalDataBase;
        for (auto& range : directory.tileByteRanges) {
            range.offset += dataDelta;
        }
        // The IFD's StripOffsets/TileOffsets tag values were written relative to the original
        // data base; shift them by the same delta so what the reader interprets matches the
        // on-disk placement the sinks write to.
        for (auto& entry : images[i].entries) {
            if (entry.tagId == tagId(TagId::StripOffsets) ||
                entry.tagId == tagId(TagId::TileOffsets)) {
                for (auto& value : entry.values) {
                    value += static_cast<std::uint32_t>(dataDelta);
                }
            }
        }
        if (directory.stripByteCountsPatchOffset != 0) {
            // The planner computed this as an absolute offset assuming the IFD began at
            // headerSize; shift it by the image's actual IFD position.
            directory.stripByteCountsPatchOffset += ifdOffsets[i] - headerSize;
        }
    }

    return TiffFileWritePlan{.images = std::move(images), .isBigTiff = isBigTiff};
}

} // namespace ptiff::io::backend::tiff
