#pragma once

#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>

#include <ptiff/core/id.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

class Image;
struct ImageDescriptor;

/// A scene: the composition of multiple related images (e.g. a stereo pair, a mosaic).
///
/// Move-only owning container of @ref ptiff::Image "Image" objects. Each image is given a
/// monotonically increasing @ref ptiff::ImageId "ImageId" when added (0, 1, 2, ...). Copy is
/// intentionally disabled; a Scene is cheap to move.
///
/// @section scene_note Round-trip
/// A Scene round-trips through io::StorageModel. Not every Image field survives a TIFF
/// round-trip: tileInfo and groundSampleDistanceMeters are not preserved.
///
/// @section scene_example Example
///
/// @code{.cpp}
/// using ptiff::Scene;
/// using ptiff::ImageDescriptor;
///
/// ImageDescriptor d;
/// d.width = 64;
/// d.height = 32;
///
/// Scene scene;
/// auto id = scene.addImage(d);       // Result<ImageId>; id == 0
/// assert(id.has_value());
/// auto img = scene.image(*id);       // Result<std::reference_wrapper<const Image>>
/// assert(img.has_value() && img->get().width() == 64);
/// @endcode
class PTIFF_EXPORT Scene {
public:
    Scene();
    ~Scene();

    Scene(const Scene&) = delete;
    Scene& operator=(const Scene&) = delete;
    Scene(Scene&&) noexcept;
    Scene& operator=(Scene&&) noexcept;

    /// @brief Appends an image built from \p descriptor and returns its new ImageId.
    /// @param descriptor Everything needed to construct the new @ref ptiff::Image "Image".
    /// @return The new @ref ptiff::ImageId "ImageId" (the next value of the per-Scene counter
    ///         starting at 0) on success.
    [[nodiscard]] Result<ImageId> addImage(const ImageDescriptor& descriptor);

    /// @brief Number of images in the scene.
    [[nodiscard]] Result<std::size_t> imageCount() const;

    /// @brief Looks up an image by its @ref ptiff::ImageId "ImageId".
    /// @param id The id returned by addImage.
    /// @return A const reference to the image on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p id was never returned by
    ///         addImage.
    [[nodiscard]] Result<std::reference_wrapper<const Image>> image(ImageId id) const;

    /// @brief Returns the image at \p index in scene order (the order they were added).
    /// @param index A zero-based index in `[0, imageCount())`.
    /// @return A const reference to the image on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p index is not in range.
    [[nodiscard]] Result<std::reference_wrapper<const Image>> imageAt(std::size_t index) const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
