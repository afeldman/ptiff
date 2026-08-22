#pragma once

namespace ptiff {

/// @brief A 3D double-precision vector (position, translation, direction, ...).
///
/// Storage-only POD: holds an x/y/z triple and provides defaulted equality. No linear algebra
/// operations (addition, cross product, ...) are provided yet -- the type is deliberately kept
/// minimal and self-contained rather than pulling in an external math dependency.
///
/// @section vec3_example Example
///
/// @code{.cpp}
/// using ptiff::Vec3;
/// Vec3 origin;                      // (0, 0, 0) by default
/// Vec3 pos{1.0, 2.0, 3.0};
/// assert(pos == Vec3(1.0, 2.0, 3.0));
/// @endcode
///
/// @note The default-initialised value is the zero vector. Value compare between equal vectors
///       is exact (no epsilon tolerance).
struct Vec3 {
    double x = 0.0; ///< X coordinate (right / east).
    double y = 0.0; ///< Y coordinate (down / north).
    double z = 0.0; ///< Z coordinate (into / up).

    friend constexpr bool operator==(const Vec3&, const Vec3&) = default;
};

} // namespace ptiff
