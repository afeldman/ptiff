#include <functional>
#include <map>
#include <utility>

#include <ptiff/annotation.hpp>

namespace ptiff {

struct Annotation::Impl {
    AnnotationKind kind;
    std::optional<ImageId> sourceImage;
    std::map<std::string, std::string, std::less<>> parameters;
};

Annotation::Annotation(AnnotationKind kind, std::optional<ImageId> sourceImage)
    : impl_(std::make_unique<Impl>(Impl{kind, sourceImage, {}})) {}

Annotation::~Annotation() = default;
Annotation::Annotation(Annotation&&) noexcept = default;
Annotation& Annotation::operator=(Annotation&&) noexcept = default;

AnnotationKind Annotation::kind() const noexcept {
    return impl_->kind;
}

std::optional<ImageId> Annotation::sourceImage() const noexcept {
    return impl_->sourceImage;
}

Result<std::string> Annotation::parameter(std::string_view key) const {
    const auto it = impl_->parameters.find(key);
    if (it == impl_->parameters.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "Annotation::parameter: no such key"});
    }
    return it->second;
}

void Annotation::setParameter(std::string_view key, std::string value) {
    impl_->parameters.insert_or_assign(std::string{key}, std::move(value));
}

} // namespace ptiff
