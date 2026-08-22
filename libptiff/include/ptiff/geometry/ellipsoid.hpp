#pragma once

namespace ptiff {

/// @brief A biaxial reference ellipsoid (sufficient for every currently-supported planetary body).
///
/// Describes the shape of a planetary body as a rotationally-symmetric ellipsoid of revolution
/// (an oblate spheroid): a semi-major (equatorial) and a semi-minor (polar) axis, both in
/// meters. Triaxial bodies (three distinct axes) are a documented future extension and are not
/// modelled yet.
///
/// @section ellipsoid_example Example
///
/// @code{.cpp}
/// using ptiff::Ellipsoid;
///
/// // IAU_MOON reference ellipsoid (approximate values, in meters).
/// Ellipsoid moon{1738100.0, 1736000.0};
///
/// // IAU_MARS biaxial reference ellipsoid (in meters).
/// Ellipsoid mars{3396190.0, 3376200.0};
/// @endcode
///
/// @note A `Ellipsoid` with both axes zero (`semiMajorAxisMeters == semiMinorAxisMeters == 0`)
///       is the degenerate/unknown case and should be interpreted as "not specified".
struct Ellipsoid {
    double semiMajorAxisMeters = 0.0; ///< Equatorial radius of the body, in meters.
    double semiMinorAxisMeters = 0.0; ///< Polar radius of the body, in meters.

    friend constexpr bool operator==(const Ellipsoid&, const Ellipsoid&) = default;
};

} // namespace ptiff
