#pragma once

#include <cstdint>
#include <memory>
#include <optional>

#include <ptiff/export.hpp>
#include <ptiff/image/image_descriptor.hpp>

namespace ptiff {

/// @brief A single scientific raster image: geometry and storage metadata, not pixel data.
///
/// Owns the descriptive metadata of one image (dimensions, pixel type, channel count, optional
/// ground sampling distance, tile layout and compression scheme). It does **not** hold pixel
/// data -- those live in the storage backend and are accessed via the IO layer.
///
/// There is no `id()` accessor -- Image identity is scoped to whichever
/// @ref ptiff::Scene "Scene" it was added to; see @ref ptiff::Scene::addImage "Scene::addImage".
///
/// @section image_example Example
///
/// @code{.cpp}
/// using ptiff::Image;
/// using ptiff::ImageDescriptor;
/// using ptiff::PixelType;
///
/// ImageDescriptor d;
/// d.width  = 128;
/// d.height = 64;
/// d.pixelType = PixelType::UInt8;
/// d.channelCount = 3;
///
/// Image img{d};
/// assert(img.width()  == 128);
/// assert(img.height() == 64);
/// @endcode
///
/// @see @ref ptiff::Scene "Scene" for the owning container of images and their ids.
class PTIFF_EXPORT Image {
public:
    /// @brief Constructs an image from its metadata descriptor.
    ///
    /// @param descriptor The @ref ptiff::ImageDescriptor "ImageDescriptor" carrying the image's
    ///                   geometry and storage metadata.
    explicit Image(ImageDescriptor descriptor);
    ~Image();

    /// Image is move-only: copying would leave two owners of the same metadata.
    Image(const Image&) = delete;
    Image& operator=(const Image&) = delete;
    Image(Image&&) noexcept;
    Image& operator=(Image&&) noexcept;

    /// @brief Returns the image width in pixels.
    [[nodiscard]] std::uint32_t width() const noexcept;

    /// @brief Returns the image height in pixels.
    [[nodiscard]] std::uint32_t height() const noexcept;

    /// @brief Returns the sample type of each channel.
    [[nodiscard]] PixelType pixelType() const noexcept;

    /// @brief Returns the number of channels per pixel.
    [[nodiscard]] std::uint32_t channelCount() const noexcept;

    /// @brief Returns the optional ground-sample distance in meters, if specified.
    ///
    /// @return `std::nullopt` if no ground-sample distance was supplied for this image.
    [[nodiscard]] std::optional<double> groundSampleDistanceMeters() const noexcept;

    /// @brief Returns the optional tile layout in pixels, if specified.
    ///
    /// @return `std::nullopt` if the image is stored as a contiguous strip (untiled).
    [[nodiscard]] std::optional<TileInfo> tileInfo() const noexcept;

    /// @brief Returns the optional compression scheme, if specified.
    ///
    /// @return `std::nullopt` if no compression is declared for this image.
    [[nodiscard]] std::optional<CompressionKind> compression() const noexcept;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
