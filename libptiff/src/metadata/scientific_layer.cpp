#include <string>

#include <ptiff/metadata/scientific_layer.hpp>

namespace ptiff {

struct ScientificLayer::Impl {};

ScientificLayer::ScientificLayer() : impl_(std::make_unique<Impl>()) {}
ScientificLayer::~ScientificLayer() = default;
ScientificLayer::ScientificLayer(ScientificLayer&&) noexcept = default;
ScientificLayer& ScientificLayer::operator=(ScientificLayer&&) noexcept = default;

Result<std::string> ScientificLayer::name() const {
    return std::unexpected(
        Error{ErrorCode::NotImplemented, "ScientificLayer::name is not implemented yet"});
}

} // namespace ptiff
