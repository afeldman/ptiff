#pragma once

#include <cstdint>
#include <optional>

#include <ptiff/image/compression_kind.hpp>
#include <ptiff/image/pixel_type.hpp>
#include <ptiff/image/tile_info.hpp>

namespace ptiff {

/// @brief Everything needed to construct an @ref ptiff::Image "Image".
///
/// A separate struct (rather than a long constructor parameter list) so call sites read as named
/// fields. Pass it to @ref ptiff::Scene::addImage "Scene::addImage" to create an image in a
/// scene.
///
/// @section image_descriptor_example Example
///
/// @code{.cpp}
/// using ptiff::ImageDescriptor;
/// using ptiff::PixelType;
/// using ptiff::CompressionKind;
/// using ptiff::TileInfo;
///
/// ImageDescriptor d;
/// d.width         = 64;
/// d.height        = 32;
/// d.pixelType     = PixelType::UInt16;
/// d.channelCount  = 1;
/// d.compression   = CompressionKind::Deflate;
/// d.tileInfo      = TileInfo{256, 256};  // optional; tiles only if presence
/// // groundSampleDistanceMeters is std::nullopt => global scale is not specified.
/// @endcode
///
/// @note `tileInfo` and `groundSampleDistanceMeters` are optional: a `std::nullopt` value means
///       "not specified". They are **not** preserved through a TIFF round-trip
///       (see @ref ptiff::Scene "Scene"'s round-trip note).
struct ImageDescriptor {
    std::uint32_t width = 0;                          ///< Image width in pixels.
    std::uint32_t height = 0;                         ///< Image height in pixels.
    PixelType pixelType = PixelType::UInt8;           ///< Sample type of each channel.
    std::uint32_t channelCount = 1;                   ///< Channels per pixel (samples/pixel).
    std::optional<double> groundSampleDistanceMeters; ///< Optional ground-sample distance, meters.
    std::optional<TileInfo> tileInfo;                 ///< Optional tiling layout in pixels.
    std::optional<CompressionKind> compression;       ///< Optional compression scheme.
};

} // namespace ptiff
