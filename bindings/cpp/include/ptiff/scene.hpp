#pragma once

/// @file scene.hpp
/// @brief A `Scene` is the owning container of `Image`s.
///
/// `Scene` owns its images and assigns each an `ImageId` in insertion order
/// (0-based). It is move-only. `addImage` takes an `ImageDescriptor` (the
/// in-memory metadata model) and builds an `Image` over the C-ABI handle.

#include <functional>
#include <memory>
#include <vector>

#include <ptiff/core.hpp>
#include <ptiff/image.hpp>

namespace ptiff {

/// @brief Owning container of images.
///
/// Mirrors the historical `ptiff::Scene`: `addImage` records an image's
/// metadata and returns its `ImageId`; `image(id)`/`imageAt(index)` return
/// const references. The scene does not hold pixel data.
class Scene {
public:
    Scene() = default;
    ~Scene() = default;
    Scene(const Scene&) = delete;
    Scene& operator=(const Scene&) = delete;
    Scene(Scene&&) noexcept = default;
    Scene& operator=(Scene&&) noexcept = default;

    /// @brief Adds an image described by `descriptor` and returns its id.
    [[nodiscard]] Result<ImageId> addImage(const ImageDescriptor& descriptor) {
        if (!descriptor.tileInfo.has_value() && descriptor.compression.has_value() &&
            descriptor.compression == CompressionKind::Jpeg) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                            "Scene::addImage: JPEG requires a tile layout"});
        }
        auto img = std::make_unique<Image>(descriptor);
        ImageId id{images_.size()};
        images_.push_back(std::move(img));
        return id;
    }

    /// @brief Number of images currently in the scene.
    [[nodiscard]] Result<std::size_t> imageCount() const noexcept {
        return images_.size();
    }

    /// @brief Returns a const reference to the image with the given id.
    [[nodiscard]] Result<std::reference_wrapper<const Image>> image(ImageId id) const {
        if (id.value() >= images_.size()) {
            return std::unexpected(Error{ErrorCode::OutOfRange,
                                            "Scene::image: id out of range"});
        }
        return std::cref(*images_[static_cast<std::size_t>(id.value())]);
    }

    /// @brief Returns a const reference to the image at the given index.
    [[nodiscard]] Result<std::reference_wrapper<const Image>> imageAt(std::size_t index) const {
        if (index >= images_.size()) {
            return std::unexpected(Error{ErrorCode::OutOfRange,
                                            "Scene::imageAt: index out of range"});
        }
        return std::cref(*images_[index]);
    }

private:
    std::vector<std::unique_ptr<Image>> images_;
};

} // namespace ptiff
