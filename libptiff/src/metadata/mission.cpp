#include <utility>

#include <ptiff/metadata/mission.hpp>

namespace ptiff {

struct Mission::Impl {
    MissionDescriptor descriptor;
};

Mission::Mission(MissionDescriptor descriptor)
    : impl_(std::make_unique<Impl>(Impl{std::move(descriptor)})) {}

Mission::~Mission() = default;
Mission::Mission(Mission&&) noexcept = default;
Mission& Mission::operator=(Mission&&) noexcept = default;

std::string_view Mission::name() const noexcept {
    return impl_->descriptor.name;
}

std::string_view Mission::agency() const noexcept {
    return impl_->descriptor.agency;
}

std::string_view Mission::instrument() const noexcept {
    return impl_->descriptor.instrument;
}

std::chrono::system_clock::time_point Mission::acquisitionTime() const noexcept {
    return impl_->descriptor.acquisitionTime;
}

std::string_view Mission::orbit() const noexcept {
    return impl_->descriptor.orbit;
}

} // namespace ptiff
