#include <functional>
#include <map>
#include <string>
#include <utility>

#include <ptiff/geometry/lens_model.hpp>

namespace ptiff {

struct LensModel::Impl {
    LensModelKind kind;
    std::map<std::string, double, std::less<>> parameters;
};

LensModel::LensModel(LensModelKind kind) : impl_(std::make_unique<Impl>(Impl{kind, {}})) {}
LensModel::~LensModel() = default;
LensModel::LensModel(LensModel&&) noexcept = default;
LensModel& LensModel::operator=(LensModel&&) noexcept = default;

LensModelKind LensModel::kind() const noexcept {
    return impl_->kind;
}

Result<double> LensModel::parameter(std::string_view key) const {
    const auto it = impl_->parameters.find(key);
    if (it == impl_->parameters.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "LensModel::parameter: no such key"});
    }
    return it->second;
}

void LensModel::setParameter(std::string_view key, double value) {
    impl_->parameters.insert_or_assign(std::string{key}, value);
}

} // namespace ptiff
