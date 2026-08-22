#pragma once

#include <memory>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief Which projection a @ref ptiff::Camera "Camera"'s @ref ptiff::Intrinsics "Intrinsics"
///        should be interpreted under.
///
/// New kinds are added here as new enum values -- never as a new C++ type -- so the Go/Python/
/// Rust bindings never need to learn a new type for a new lens model.
enum class LensModelKind {
    Pinhole,   ///< Perspective (pinhole) projection. Distortion params optional.
    Fisheye,   ///< Wide-angle fisheye projection.
    Pushbroom, ///< Line-scan (pushbroom) sensor projection.
};

/// @brief Named numeric parameters for a lens model (e.g. `"k1"`/`"k2"`/`"p1"` distortion
///        coefficients).
///
/// Holds a @ref ptiff::LensModelKind "LensModelKind" plus an optional set of named double
/// parameters. Which keys are meaningful depends on `kind()`.
///
/// @section lens_model_example Example
///
/// @code{.cpp}
/// using ptiff::LensModel;
/// using ptiff::LensModelKind;
///
/// LensModel m(LensModelKind::Pinhole);
/// m.setParameter("k1", -0.1);
/// m.setParameter("k2", 0.05);
///
/// auto k1 = m.parameter("k1");
/// if (k1) assert(*k1 == -0.1);
/// @endcode
///
/// @see @ref ptiff::Camera "Camera" for the owning camera; @ref ptiff::Intrinsics "Intrinsics"
///      for the corresponding un-distorted sensor parameters.
class PTIFF_EXPORT LensModel {
public:
    /// @brief Constructs a lens model of the given kind.
    ///
    /// @param kind The @ref ptiff::LensModelKind "LensModelKind" of this lens model.
    explicit LensModel(LensModelKind kind);
    ~LensModel();

    LensModel(const LensModel&) = delete;
    LensModel& operator=(const LensModel&) = delete;
    LensModel(LensModel&&) noexcept;
    LensModel& operator=(LensModel&&) noexcept;

    /// @brief Returns the lens model kind.
    [[nodiscard]] LensModelKind kind() const noexcept;

    /// @brief Looks up a named parameter.
    ///
    /// @param key The parameter name (e.g. `"k1"`).
    /// @return The parameter value on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p key was never set.
    [[nodiscard]] Result<double> parameter(std::string_view key) const;

    /// @brief Sets a named parameter.
    ///
    /// @param key   The parameter name (e.g. `"k1"`).
    /// @param value The parameter value.
    void setParameter(std::string_view key, double value);

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
