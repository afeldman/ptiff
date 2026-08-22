#include <string>

#include <ptiff/metadata/metadata.hpp>

namespace ptiff {

struct Metadata::Impl {};

Metadata::Metadata() : impl_(std::make_unique<Impl>()) {}
Metadata::~Metadata() = default;
Metadata::Metadata(Metadata&&) noexcept = default;
Metadata& Metadata::operator=(Metadata&&) noexcept = default;

Result<std::string> Metadata::get(std::string_view /*key*/) const {
    return std::unexpected(
        Error{ErrorCode::NotImplemented, "Metadata::get is not implemented yet"});
}

} // namespace ptiff
