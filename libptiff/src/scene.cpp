#include <cstdint>
#include <utility>
#include <vector>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

struct Scene::Impl {
    std::vector<Image> images;
    std::uint64_t nextId = 0;
};

Scene::Scene() : impl_(std::make_unique<Impl>()) {}
Scene::~Scene() = default;
Scene::Scene(Scene&& other) noexcept : impl_(std::move(other.impl_)) {
    other.impl_ = std::make_unique<Impl>();
}

Scene& Scene::operator=(Scene&& other) noexcept {
    if (this != &other) {
        impl_ = std::move(other.impl_);
        other.impl_ = std::make_unique<Impl>();
    }
    return *this;
}

Result<ImageId> Scene::addImage(const ImageDescriptor& descriptor) {
    ImageId id{impl_->nextId};
    impl_->images.emplace_back(Image(descriptor));
    ++impl_->nextId;
    return id;
}

Result<std::size_t> Scene::imageCount() const {
    return impl_->images.size();
}

Result<std::reference_wrapper<const Image>> Scene::image(ImageId id) const {
    const auto index = static_cast<std::size_t>(id.value());
    if (index >= impl_->images.size()) {
        return std::unexpected(Error{ErrorCode::NotFound, "Scene::image: unknown image id"});
    }
    return std::cref(impl_->images[index]);
}

Result<std::reference_wrapper<const Image>> Scene::imageAt(std::size_t index) const {
    if (index >= impl_->images.size()) {
        return std::unexpected(Error{ErrorCode::OutOfRange, "Scene::imageAt: index out of range"});
    }
    return std::cref(impl_->images[index]);
}

} // namespace ptiff
