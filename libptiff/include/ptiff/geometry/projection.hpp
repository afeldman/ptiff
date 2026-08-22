#pragma once

#include <memory>
#include <string>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief Which map projection a @ref ptiff::CoordinateReferenceSystem "CoordinateReferenceSystem"
///        uses.
///
/// New kinds are added here as new enum values -- never as a new C++ type -- matching
/// @ref ptiff::LensModel "LensModel"'s and @ref ptiff::ScientificLayer "ScientificLayer"'s
/// extension pattern.
enum class ProjectionKind {
    Equirectangular, ///< Equirectangular (plate carrée) projection.
    Stereographic,   ///< Stereographic projection.
    Sinusoidal,      ///< Sinusoidal (Sanson–Flamsteed) projection.
    Orthographic,    ///< Azimuthal orthographic projection.
};

/// @brief Named parameters for a projection (e.g. `"central_meridian"`, `"standard_parallel"`).
///
/// Holds a @ref ptiff::ProjectionKind "ProjectionKind" plus an optional set of named
/// double parameters. Which keys are meaningful depends on `kind()`.
///
/// @section projection_example Example
///
/// @code{.cpp}
/// using ptiff::Projection;
/// using ptiff::ProjectionKind;
///
/// Projection p(ProjectionKind::Equirectangular);
/// p.setParameter("central_meridian", 0.0);
///
/// auto v = p.parameter("central_meridian");
/// if (v) assert(*v == 0.0);
/// @endcode
///
/// @see @ref ptiff::CoordinateReferenceSystem "CoordinateReferenceSystem" for the owning CRS.
class PTIFF_EXPORT Projection {
public:
    /// @brief Constructs a projection of the given kind.
    ///
    /// @param kind The @ref ptiff::ProjectionKind "ProjectionKind" of this projection.
    explicit Projection(ProjectionKind kind);
    ~Projection();

    Projection(const Projection&) = delete;
    Projection& operator=(const Projection&) = delete;
    Projection(Projection&&) noexcept;
    Projection& operator=(Projection&&) noexcept;

    /// @brief Returns the projection kind.
    [[nodiscard]] ProjectionKind kind() const noexcept;

    /// @brief Looks up a named parameter.
    ///
    /// @param key The parameter name (e.g. `"central_meridian"`).
    /// @return The parameter value on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p key was never set.
    [[nodiscard]] Result<double> parameter(std::string_view key) const;

    /// @brief Sets a named parameter.
    ///
    /// @param key   The parameter name (e.g. `"central_meridian"`).
    /// @param value The parameter value.
    void setParameter(std::string_view key, double value);

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
