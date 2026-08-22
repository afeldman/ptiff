#pragma once

namespace ptiff {

/// @brief A rotation represented as a unit quaternion (w, x, y, z).
///
/// Storage-only POD: holds four double components and provides defaulted equality. Unlike a
/// `Vec3`, *no* normalization is performed on construction -- a caller is responsible for
/// supplying a unit quaternion. Composition/inversion operations are not implemented yet.
///
/// @section quaternion_convention Convention
/// The real part is stored first (`w`), followed by the imaginary parts (`x`, `y`, `z`) -- the
/// Hamiltonian convention `q = w + x·i + y·j + z·k`.
///
/// @section quaternion_example Example
///
/// @code{.cpp}
/// using ptiff::Quaternion;
/// Quaternion identity;                 // 1 + 0i + 0j + 0k
/// Quaternion rot{0.0, 1.0, 0.0, 0.0}; // 180° about the +x axis
/// @endcode
///
/// @note The default value is the identity quaternion `(1, 0, 0, 0)`.
struct Quaternion {
    double w = 1.0; ///< Real part.
    double x = 0.0; ///< Imaginary component of the x axis.
    double y = 0.0; ///< Imaginary component of the y axis.
    double z = 0.0; ///< Imaginary component of the z axis.

    friend constexpr bool operator==(const Quaternion&, const Quaternion&) = default;
};

} // namespace ptiff
