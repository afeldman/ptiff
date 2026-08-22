#pragma once

#include <ptiff/geometry/quaternion.hpp>
#include <ptiff/geometry/vector3.hpp>

namespace ptiff {

/// @brief A camera's pose: rotation (camera-to-world) and translation (world position).
///
/// Describes the 6-DOF exterior orientation of a sensor: a unit @ref ptiff::Quaternion
/// "Quaternion" rotating from the *camera* frame into the *world* frame, plus a @ref ptiff::Vec3
/// "Vec3" giving the camera position in world coordinates.
///
/// @section extrinsics_example Example
///
/// @code{.cpp}
/// using ptiff::Extrinsics;
/// using ptiff::Quaternion;
/// using ptiff::Vec3;
///
/// Extrinsics e;
/// e.rotation  = Quaternion{0.7071, -0.7071, 0.0, 0.0}; // ~90° about -x
/// e.translation = Vec3{100.0, 50.0, 10.0};             // camera position in world meters
/// @endcode
struct Extrinsics {
    Quaternion rotation; ///< Camera-to-world rotation (unit quaternion).
    Vec3 translation;    ///< Camera position in world coordinates (meters).

    friend constexpr bool operator==(const Extrinsics&, const Extrinsics&) = default;
};

} // namespace ptiff
