#include <functional>
#include <map>
#include <utility>

#include <ptiff/geometry.hpp>

namespace ptiff {

struct Geometry::Impl {
    GeometryKind kind;
    std::optional<ImageId> sourceImage;
    std::map<std::string, std::string, std::less<>> parameters;
};

Geometry::Geometry(GeometryKind kind, std::optional<ImageId> sourceImage)
    : impl_(std::make_unique<Impl>(Impl{kind, sourceImage, {}})) {}

Geometry::~Geometry() = default;
Geometry::Geometry(Geometry&&) noexcept = default;
Geometry& Geometry::operator=(Geometry&&) noexcept = default;

GeometryKind Geometry::kind() const noexcept {
    return impl_->kind;
}

std::optional<ImageId> Geometry::sourceImage() const noexcept {
    return impl_->sourceImage;
}

Result<std::string> Geometry::parameter(std::string_view key) const {
    const auto it = impl_->parameters.find(key);
    if (it == impl_->parameters.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "Geometry::parameter: no such key"});
    }
    return it->second;
}

void Geometry::setParameter(std::string_view key, std::string value) {
    impl_->parameters.insert_or_assign(std::string{key}, std::move(value));
}

} // namespace ptiff
