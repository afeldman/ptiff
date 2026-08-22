#include <string>

#include <ptiff/geometry/coordinate_reference_system.hpp>

namespace ptiff {

struct CoordinateReferenceSystem::Impl {};

CoordinateReferenceSystem::CoordinateReferenceSystem() : impl_(std::make_unique<Impl>()) {}
CoordinateReferenceSystem::~CoordinateReferenceSystem() = default;
CoordinateReferenceSystem::CoordinateReferenceSystem(CoordinateReferenceSystem&&) noexcept =
    default;
CoordinateReferenceSystem&
CoordinateReferenceSystem::operator=(CoordinateReferenceSystem&&) noexcept = default;

Result<std::string> CoordinateReferenceSystem::identifier() const {
    return std::unexpected(Error{ErrorCode::NotImplemented,
                                 "CoordinateReferenceSystem::identifier is not implemented yet"});
}

} // namespace ptiff
