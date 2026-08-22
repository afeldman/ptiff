#pragma once

// Model <-> Imf::Header mapping shared by the OpenExrBackend methods.
//
// OpenEXR is itself a self-describing container, so unlike PDS4/ISIS (which wrap a hand-rolled
// text/XML label around a raw pixel block), the OpenExrBackend has no separate ptiff-level label
// format at all: the document *is* a real, standalone .exr file, read/written entirely through
// the genuine Imf:: API. serializeModel()/deserializeModel() read and write nothing but the
// Imf::Header; openImageSource()/openImageSink() own the full read/write session (header +
// pixel chunks), see openexr_image_source.hpp / openexr_image_sink.hpp for why that split exists.
//
// Pixel type support this phase is intentionally narrow: only the two ptiff PixelTypes that map
// 1:1 onto a native OpenEXR channel PixelType are supported (Float32 -> Imf::FLOAT, UInt32 ->
// Imf::UINT). UInt8/UInt16/Float64 and OpenEXR's native HALF channels are deferred -- promoting
// narrower/wider ptiff types into an OpenEXR channel type would need either data-widening or a
// side-channel to recover the original width on read, which is unwarranted complexity for this
// phase. Samples-per-pixel is mapped to channel names the same way common OpenEXR tooling does:
// 1 -> "Y" (luminance), 3 -> "R","G","B", 4 -> "R","G","B","A"; other counts are unsupported.

#include <cstdint>
#include <string>
#include <vector>

#include <ImfPixelType.h>

#include <ptiff/core/result.hpp>
#include <ptiff/image.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::openexr {

/// @brief Resolved per-image metadata for the OpenEXR backend.
struct OpenExrImageInfo {
    std::uint32_t width = 0;
    std::uint32_t height = 0;
    std::uint32_t samplesPerPixel = 0;
    ptiff::PixelType pixelType = ptiff::PixelType::Float32;
};

/// @brief Maps a ptiff::PixelType onto the native Imf::PixelType it round-trips through, if any.
[[nodiscard]] Result<Imf::PixelType> toImfPixelType(ptiff::PixelType type);
/// @brief Inverse of @ref toImfPixelType.
[[nodiscard]] Result<ptiff::PixelType> fromImfPixelType(Imf::PixelType type);

/// @brief Channel names (in storage order) for a given samplesPerPixel (1, 3 or 4).
[[nodiscard]] Result<std::vector<std::string>> channelNamesFor(std::uint32_t samplesPerPixel);

/// @brief Parses the flat per-image storage fields (imageWidth/imageHeight/samplesPerPixel/
///        pixelType) out of @p model. Optional tileWidth/tileHeight fields, if present, must
///        equal imageWidth/imageHeight (the backend only supports a single whole-image tile).
[[nodiscard]] Result<OpenExrImageInfo> imageInfoFromModel(const StorageModel& model);

/// @brief Builds the flat per-image storage fields (imageWidth/imageHeight/tileWidth/tileHeight/
///        samplesPerPixel/pixelType/compression) StorageModel for @p info.
[[nodiscard]] StorageModel modelFromImageInfo(const OpenExrImageInfo& info);

/// @brief Writes a header-only, real, valid .exr document (magic + version + header, zero
///        pixel chunks) for @p model to @p writer, starting at offset 0.
[[nodiscard]] Result<void> writeHeaderOnly(const StorageModel& model, io::BinaryWriter& writer);

/// @brief Reads @p reader (from offset 0) as an .exr document and returns its resolved image
///        info. Only the header is read; pixel chunks are untouched.
[[nodiscard]] Result<OpenExrImageInfo> readHeaderInfo(io::BinaryReader& reader);

} // namespace ptiff::io::backend::openexr
