#pragma once

/// @file image.hpp
/// @brief Image metadata types + the RAII `ptiff::Image` wrapper.
///
/// An `Image` owns the descriptive metadata of one raster image (dimensions,
/// pixel type, channel count, optional GSD, tile layout, compression). It
/// does *not* hold pixel data — those live in the storage backend and are
/// read/written through `Reader`/`Writer`. The class is a thin RAII shell
/// over the opaque `struct ptiff_image` handle returned by `ptiff_image_create`.

#include <cstdint>
#include <memory>
#include <optional>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp> // extern "C" declarations + structs, types

namespace ptiff {

/// @brief Mirror of the C `ptiff_pixel_type` enum (additive).
enum class PixelType {
    UInt8,   ///< Unsigned 8-bit sample.
    UInt16,  ///< Unsigned 16-bit sample.
    UInt32,  ///< Unsigned 32-bit sample.
    Float32, ///< IEEE-754 32-bit float.
    Float64, ///< IEEE-754 64-bit float.
};

/// @brief Mirror of the C `ptiff_compression_kind` enum (additive).
enum class CompressionKind {
    None,    ///< No compression (raw/lossless).
    Lzw,     ///< TIFF-variant LZW.
    Deflate, ///< zlib-wrapped Deflate.
    Jpeg,    ///< Baseline JPEG (8-bit only).
};

/// @brief Tile layout in pixels.
struct TileInfo {
    std::uint32_t tileWidth = 0;
    std::uint32_t tileHeight = 0;
    friend constexpr bool operator==(const TileInfo&, const TileInfo&) = default;
};

/// @brief Descriptive metadata for one image (the in-memory model).
struct ImageDescriptor {
    std::uint32_t width = 0;
    std::uint32_t height = 0;
    PixelType pixelType = PixelType::UInt8;
    std::uint32_t channelCount = 1;
    std::optional<double> groundSampleDistanceMeters;
    std::optional<TileInfo> tileInfo;
    std::optional<CompressionKind> compression;
};

namespace detail {

// Maps the C ABI value struct to/from the idiomatic model.
inline detail::ptiff_image_descriptor toCDescriptor(const ImageDescriptor& d) {
    detail::ptiff_image_descriptor c{};
    c.width = d.width;
    c.height = d.height;
    c.pixel_type = static_cast<int32_t>(d.pixelType);
    c.channel_count = d.channelCount;
    if (d.groundSampleDistanceMeters.has_value()) {
        c.has_gsd = 1;
        c.gsd = *d.groundSampleDistanceMeters;
    }
    if (d.tileInfo.has_value()) {
        c.has_tile_info = 1;
        c.tile_info.tile_width = d.tileInfo->tileWidth;
        c.tile_info.tile_height = d.tileInfo->tileHeight;
    }
    if (d.compression.has_value()) {
        c.has_compression = 1;
        c.compression = static_cast<int32_t>(*d.compression);
    }
    return c;
}

inline ImageDescriptor fromCDescriptor(const detail::ptiff_image_descriptor& c) {
    ImageDescriptor d;
    d.width = c.width;
    d.height = c.height;
    d.pixelType = static_cast<PixelType>(c.pixel_type);
    d.channelCount = c.channel_count;
    if (c.has_gsd) {
        d.groundSampleDistanceMeters = c.gsd;
    }
    if (c.has_tile_info) {
        d.tileInfo = TileInfo{c.tile_info.tile_width, c.tile_info.tile_height};
    }
    if (c.has_compression) {
        d.compression = static_cast<CompressionKind>(c.compression);
    }
    return d;
}

} // namespace detail

/// @brief RAII wrapper over the opaque `struct ptiff_image` handle.
///
/// A completed `Image` constructed from an `ImageDescriptor` holds no pixel
/// data; use `Reader::imageSource`/`Writer` to access pixels. Move-only.
class Image {
public:
    /// @brief Constructs an `Image` from its metadata descriptor.
    explicit Image(ImageDescriptor descriptor) {
        auto c = detail::toCDescriptor(descriptor);
        handle_.reset(detail::ptiff_image_create(&c));
    }

    [[nodiscard]] std::uint32_t width() const noexcept {
        return detail::ptiff_image_width(handle_.get());
    }
    [[nodiscard]] std::uint32_t height() const noexcept {
        return detail::ptiff_image_height(handle_.get());
    }
    [[nodiscard]] PixelType pixelType() const noexcept {
        return static_cast<PixelType>(detail::ptiff_image_pixel_type(handle_.get()));
    }
    [[nodiscard]] std::uint32_t channelCount() const noexcept {
        return detail::ptiff_image_channel_count(handle_.get());
    }
    [[nodiscard]] std::optional<double> groundSampleDistanceMeters() const noexcept {
        double out = 0.0;
        if (detail::ptiff_image_gsd(handle_.get(), &out) != 0) {
            return out;
        }
        return std::nullopt;
    }
    [[nodiscard]] std::optional<TileInfo> tileInfo() const noexcept {
        detail::ptiff_tile_info ti{};
        if (detail::ptiff_image_tile_info(handle_.get(), &ti) != 0) {
            return TileInfo{ti.tile_width, ti.tile_height};
        }
        return std::nullopt;
    }
    [[nodiscard]] std::optional<CompressionKind> compression() const noexcept {
        int32_t out = 0;
        if (detail::ptiff_image_compression(handle_.get(), &out) != 0) {
            return static_cast<CompressionKind>(out);
        }
        return std::nullopt;
    }

private:
    struct PtiffImageDeleter {
        void operator()(detail::ptiff_image* p) const noexcept {
            if (p) detail::ptiff_image_destroy(p);
        }
    };
    std::unique_ptr<detail::ptiff_image, PtiffImageDeleter> handle_;
};

} // namespace ptiff
