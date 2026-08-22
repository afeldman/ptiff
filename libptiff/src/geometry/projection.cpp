#include <functional>
#include <map>

#include <ptiff/geometry/projection.hpp>

namespace ptiff {

struct Projection::Impl {
    ProjectionKind kind;
    std::map<std::string, double, std::less<>> parameters;
};

Projection::Projection(ProjectionKind kind) : impl_(std::make_unique<Impl>(Impl{kind, {}})) {}
Projection::~Projection() = default;
Projection::Projection(Projection&&) noexcept = default;
Projection& Projection::operator=(Projection&&) noexcept = default;

ProjectionKind Projection::kind() const noexcept {
    return impl_->kind;
}

Result<double> Projection::parameter(std::string_view key) const {
    const auto it = impl_->parameters.find(key);
    if (it == impl_->parameters.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "Projection::parameter: no such key"});
    }
    return it->second;
}

void Projection::setParameter(std::string_view key, double value) {
    impl_->parameters.insert_or_assign(std::string{key}, value);
}

} // namespace ptiff
