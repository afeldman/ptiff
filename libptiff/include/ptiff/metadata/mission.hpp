#pragma once

#include <chrono>
#include <memory>
#include <string>
#include <string_view>

#include <ptiff/export.hpp>

namespace ptiff {

/// @brief Everything needed to construct a @ref ptiff::Mission "Mission".
///
/// A value-collection struct of capture/context fields passed to the `Mission` constructor.
struct MissionDescriptor {
    std::string name;       ///< Mission name (e.g. "Lunar Reconnaissance Orbiter").
    std::string agency;     ///< Operating agency (e.g. "NASA").
    std::string instrument; ///< Acquiring instrument (e.g. "LROC").
    std::chrono::system_clock::time_point acquisitionTime; ///< Time of acquisition.
    std::string orbit; ///< Orbit/segment identifier (free-form).
};

/// @brief Static capture context: which mission, agency, instrument, and orbit produced this
///        data, and when.
///
/// A `Mission` records the *static* context describing how a dataset was acquired. It is set
/// once and never appended to -- contrast with @ref ptiff::History "History", which records
/// what happened to the data *afterward* (an append-only provenance log).
///
/// @section mission_example Example
///
/// @code{.cpp}
/// using ptiff::Mission;
/// using ptiff::MissionDescriptor;
/// #include <chrono>
///
/// MissionDescriptor d;
/// d.name       = "Lunar Reconnaissance Orbiter";
/// d.agency     = "NASA";
/// d.instrument = "LROC-WAC";
/// d.orbit      = "orbit_123";
/// d.acquisitionTime = std::chrono::system_clock::now();
///
/// Mission m{d};
/// assert(m.name() == "Lunar Reconnaissance Orbiter");
/// @endcode
///
/// @see @ref ptiff::History "History" for the append-only provenance log.
class PTIFF_EXPORT Mission {
public:
    /// @brief Constructs a mission from its descriptor.
    ///
    /// @param descriptor The @ref ptiff::MissionDescriptor "MissionDescriptor" carrying the
    ///                   capture context.
    explicit Mission(MissionDescriptor descriptor);
    ~Mission();

    Mission(const Mission&) = delete;
    Mission& operator=(const Mission&) = delete;
    Mission(Mission&&) noexcept;
    Mission& operator=(Mission&&) noexcept;

    /// @brief Returns the mission name (e.g. `"Lunar Reconnaissance Orbiter"`).
    [[nodiscard]] std::string_view name() const noexcept;

    /// @brief Returns the operating agency (e.g. `"NASA"`).
    [[nodiscard]] std::string_view agency() const noexcept;

    /// @brief Returns the acquiring instrument (e.g. `"LROC"`).
    [[nodiscard]] std::string_view instrument() const noexcept;

    /// @brief Returns the time of acquisition.
    [[nodiscard]] std::chrono::system_clock::time_point acquisitionTime() const noexcept;

    /// @brief Returns the orbit/segment identifier (free-form).
    [[nodiscard]] std::string_view orbit() const noexcept;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
