#pragma once

#include <string>
#include <string_view>
#include <utility>

#include <ptiff/geometry/ellipsoid.hpp>

namespace ptiff {

/// @brief A planetary body: identity and shape.
///
/// Describes one body that PTIFF datasets may be georeferenced against (Earth, the Moon, Mars,
/// ...). A `Planet` is an immutable value object assembled from its name, IAU identifier, shape
/// (@ref ptiff::Ellipsoid "Ellipsoid") and the body's canonical IAU default reference frame.
///
/// New bodies are just new `Planet` instances -- there is deliberately **no enum-of-planets** to
/// extend, so adding a body is a runtime concern rather than a library-code change.
///
/// @section planet_example Example
///
/// @code{.cpp}
/// using ptiff::Planet;
/// using ptiff::Ellipsoid;
///
/// // IAU_MOON (approximate lunar radius, meters).
/// Planet moon("Moon", "301", Ellipsoid{1738100.0, 1736000.0}, "IAU_MOON");
///
/// auto name  = moon.name();            // "Moon"
/// auto iauId = moon.iauIdentifier();   // "301"
/// auto frame = moon.referenceFrame();  // "IAU_MOON"
/// @endcode
///
/// @see @ref ptiff::CoordinateReferenceSystem "CoordinateReferenceSystem" for how a dataset's
///      frame realization may differ from the body's canonical default.
class Planet {
public:
    /// @brief Constructs a planetary body.
    ///
    /// @param name           Human-readable body name (e.g. `"Moon"`).
    /// @param iauIdentifier  IAU body identifier (e.g. `"301"` for the Moon).
    /// @param ellipsoid      Reference @ref ptiff::Ellipsoid "Ellipsoid" describing the shape.
    /// @param referenceFrame Canonical IAU default frame (e.g. `"IAU_MOON"`).
    Planet(std::string name,
           std::string iauIdentifier,
           Ellipsoid ellipsoid,
           std::string referenceFrame)
        : name_(std::move(name)),
          iauIdentifier_(std::move(iauIdentifier)),
          ellipsoid_(ellipsoid),
          referenceFrame_(std::move(referenceFrame)) {}

    /// @brief Returns the human-readable body name (e.g. `"Moon"`).
    [[nodiscard]] std::string_view name() const noexcept { return name_; }

    /// @brief Returns the IAU body identifier (e.g. `"301"` for the Moon).
    [[nodiscard]] std::string_view iauIdentifier() const noexcept { return iauIdentifier_; }

    /// @brief Returns the reference @ref ptiff::Ellipsoid "Ellipsoid" describing the body's shape.
    [[nodiscard]] const Ellipsoid& ellipsoid() const noexcept { return ellipsoid_; }

    /// @brief Returns the body's canonical IAU default reference frame (e.g. `"IAU_MOON"`).
    [[nodiscard]] std::string_view referenceFrame() const noexcept { return referenceFrame_; }

private:
    std::string name_;
    std::string iauIdentifier_;
    Ellipsoid ellipsoid_;
    std::string referenceFrame_;
};

} // namespace ptiff
