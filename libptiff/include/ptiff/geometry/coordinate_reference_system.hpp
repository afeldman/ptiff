#pragma once

#include <memory>
#include <string>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief A planetary coordinate reference system: the body, frame realization, and projection.
///
/// Combines the planetary @ref ptiff::Planet "Planet" (identity/shape), an optional frame
/// realization override, and a @ref ptiff::Projection "Projection" into one object that fully
/// denotes how a dataset is georeferenced.
///
/// Sprint-1 note: this is an interface-only stub. Every accessor returns
/// @ref ptiff::ErrorCode::NotImplemented "ErrorCode::NotImplemented" until CRS support exists.
/// Move-only; copy semantics will be decided once there is real state.
///
/// @see @ref ptiff::Planet "Planet", @ref ptiff::Projection "Projection" for the composing types.
class PTIFF_EXPORT CoordinateReferenceSystem {
public:
    CoordinateReferenceSystem();
    ~CoordinateReferenceSystem();

    CoordinateReferenceSystem(const CoordinateReferenceSystem&) = delete;
    CoordinateReferenceSystem& operator=(const CoordinateReferenceSystem&) = delete;
    CoordinateReferenceSystem(CoordinateReferenceSystem&&) noexcept;
    CoordinateReferenceSystem& operator=(CoordinateReferenceSystem&&) noexcept;

    /// @brief Returns the CRS identifier (e.g. an authority code).
    ///
    /// Sprint-1 stub: returns @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @return The identifier on success, or an error otherwise.
    [[nodiscard]] Result<std::string> identifier() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
