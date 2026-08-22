#pragma once

// Layout & pixel-region helpers for the Memory ("PMEM") backend -- internal helper, not
// installed. Derives a TileLayout and per-image pixel metadata from a flat StorageModel carrying
// the scene image fields (imageWidth/imageHeight/tileWidth/tileHeight) plus samplesPerPixel and
// pixelType (see the memory-backend design spec, D3/D5), and computes the linear, checked pixel
// sizes/offsets used by the backend's sinks and sources.

#include <cstdint>
#include <exception>
#include <limits>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/image.hpp>
#include <ptiff/io/backend/memory/memory_codec.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io::backend::memory {

/// @brief Parsed pixel bytes per sample for a @ref ptiff::PixelType "PixelType".
[[nodiscard]] constexpr std::uint32_t bytesPerSample(ptiff::PixelType type) noexcept {
    switch (type) {
    case ptiff::PixelType::UInt8:
        return 1;
    case ptiff::PixelType::UInt16:
        return 2;
    case ptiff::PixelType::UInt32:
    case ptiff::PixelType::Float32:
        return 4;
    case ptiff::PixelType::Float64:
        return 8;
    }
    return 1; // unreachable, suppress -Wreturn-type
}

/// @brief Parses the string pixelType field ("UInt8".."Float64") into a @ref ptiff::PixelType.
[[nodiscard]] inline Result<ptiff::PixelType> parsePixelType(std::string_view value) {
    if (value == "UInt8") {
        return ptiff::PixelType::UInt8;
    }
    if (value == "UInt16") {
        return ptiff::PixelType::UInt16;
    }
    if (value == "UInt32") {
        return ptiff::PixelType::UInt32;
    }
    if (value == "Float32") {
        return ptiff::PixelType::Float32;
    }
    if (value == "Float64") {
        return ptiff::PixelType::Float64;
    }
    return std::unexpected(Error{ErrorCode::InvalidArgument, "memory: unrecognized pixelType"});
}

/// @brief Parses a required numeric u32 field from \p node.
[[nodiscard]] inline Result<std::uint32_t> requiredU32Field(const io::StorageModel& node,
                                                            std::string_view key) {
    auto field = node.field(key);
    if (!field.has_value()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "memory: missing required field \"" + std::string{key} + "\""});
    }
    try {
        const std::uint64_t parsed = std::stoull(*field);
        if (parsed > std::numeric_limits<std::uint32_t>::max()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "memory: field \"" + std::string{key} + "\" out of range"});
        }
        return static_cast<std::uint32_t>(parsed);
    } catch (const std::exception&) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "memory: non-numeric field \"" + std::string{key} + "\""});
    }
}

/// @brief Resolved per-image storage metadata + derived pixel geometry for the Memory backend.
struct MemoryImageInfo {
    tile::TileLayout layout;
    std::uint32_t samplesPerPixel = 0;
    ptiff::PixelType pixelType = ptiff::PixelType::UInt8;
    // Derived:
    std::uint64_t tileBytes = 0;       // tileW * tileH * spp * bytesPerSample
    std::uint64_t imagePixelBytes = 0; // rows(0) * columns(0) * tileBytes
};

namespace detail {

/// @brief Multiplies two uint64 values, rejecting instead of overflowing.
[[nodiscard]] inline Result<std::uint64_t> checkedMul(std::uint64_t a, std::uint64_t b) {
    if (a != 0 && b > std::numeric_limits<std::uint64_t>::max() / a) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: pixel geometry overflow"});
    }
    return a * b;
}

} // namespace detail

/// @brief Derives a @ref tile::TileLayout "TileLayout" plus pixel sizing from a flat image model.
///
/// @return @ref MemoryImageInfo "MemoryImageInfo" on success, or
///         `ErrorCode::InvalidArgument` if a required field is missing/non-numeric or the
///         derived geometry overflows.
[[nodiscard]] inline Result<MemoryImageInfo> imageInfoFromModel(const io::StorageModel& model) {
    auto width = requiredU32Field(model, "imageWidth");
    if (!width.has_value()) {
        return std::unexpected(width.error());
    }
    auto height = requiredU32Field(model, "imageHeight");
    if (!height.has_value()) {
        return std::unexpected(height.error());
    }
    auto tileWidth = requiredU32Field(model, "tileWidth");
    if (!tileWidth.has_value()) {
        return std::unexpected(tileWidth.error());
    }
    auto tileHeight = requiredU32Field(model, "tileHeight");
    if (!tileHeight.has_value()) {
        return std::unexpected(tileHeight.error());
    }
    if (*tileWidth == 0 || *tileHeight == 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: non-tiled images are not supported"});
    }

    auto spp = requiredU32Field(model, "samplesPerPixel");
    if (!spp.has_value()) {
        return std::unexpected(spp.error());
    }
    auto pixelTypeField = model.field("pixelType");
    if (!pixelTypeField.has_value()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "memory: missing pixelType"});
    }
    auto pixelType = parsePixelType(*pixelTypeField);
    if (!pixelType.has_value()) {
        return std::unexpected(pixelType.error());
    }

    auto compression = model.field("compression");
    if (compression.has_value() && *compression != "None") {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "memory: only uncompressed (compression==None) images are supported"});
    }

    MemoryImageInfo info;
    info.layout = tile::TileLayout{.tileSize = {.width = *tileWidth, .height = *tileHeight},
                                   .imageWidth = *width,
                                   .imageHeight = *height,
                                   .levelCount = 1};
    info.samplesPerPixel = *spp;
    info.pixelType = *pixelType;

    const std::uint64_t bps = bytesPerSample(*pixelType);
    auto perTile = detail::checkedMul(static_cast<std::uint64_t>(*tileWidth),
                                      static_cast<std::uint64_t>(*tileHeight));
    if (!perTile.has_value()) {
        return std::unexpected(perTile.error());
    }
    auto perTileSamples = detail::checkedMul(*perTile, static_cast<std::uint64_t>(*spp));
    if (!perTileSamples.has_value()) {
        return std::unexpected(perTileSamples.error());
    }
    auto tileBytes = detail::checkedMul(*perTileSamples, bps);
    if (!tileBytes.has_value()) {
        return std::unexpected(tileBytes.error());
    }
    info.tileBytes = *tileBytes;

    const std::uint64_t columns = info.layout.columns();
    const std::uint64_t rows = info.layout.rows();
    auto imagePixels = detail::checkedMul(columns, rows);
    if (!imagePixels.has_value()) {
        return std::unexpected(imagePixels.error());
    }
    auto imageBytes = detail::checkedMul(*imagePixels, info.tileBytes);
    if (!imageBytes.has_value()) {
        return std::unexpected(imageBytes.error());
    }
    info.imagePixelBytes = *imageBytes;
    return info;
}

/// @brief Returns the absolute byte offset of image \p imageIndex's pixel region within a
///        document whose model header length is \p docPrefix (see the design spec, D3).
[[nodiscard]] inline std::uint64_t imagePixelOffset(const std::vector<MemoryImageInfo>& images,
                                                    std::size_t imageIndex,
                                                    std::uint64_t docPrefix) {
    std::uint64_t offset = docPrefix;
    for (std::size_t i = 0; i < imageIndex && i < images.size(); ++i) {
        offset += images[i].imagePixelBytes;
    }
    return offset;
}

} // namespace ptiff::io::backend::memory
