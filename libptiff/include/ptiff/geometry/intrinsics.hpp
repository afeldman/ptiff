#pragma once

namespace ptiff {

/// @brief Pinhole-style intrinsic parameters for a camera, in pixels.
///
/// Describes the image-side characteristics of a sensor: the focal length (in pixel units, per
/// axis) and the principal point (the optical axis intersect with the image plane, in pixel
/// coordinates). How these values are interpreted (e.g. whether lens distortion is assumed)
/// depends on the owning @ref ptiff::Camera "Camera"'s @ref ptiff::LensModel "LensModel".
///
/// @section intrinsics_example Example
///
/// @code{.cpp}
/// using ptiff::Intrinsics;
/// Intrinsics i;
/// i.focalLengthPixelsX = 900.0;
/// i.focalLengthPixelsY = 900.0;
/// i.principalPointX = 512.0;
/// i.principalPointY = 384.0;
/// @endcode
///
/// @note All values default to `0.0`; a zero focal length does **not** denote an infinite focal
///       length but an "unset" value.
struct Intrinsics {
    double focalLengthPixelsX = 0.0; ///< Focal length along the horizontal image axis, in pixels.
    double focalLengthPixelsY = 0.0; ///< Focal length along the vertical image axis, in pixels.
    double principalPointX = 0.0;    ///< Principal point X coordinate, in pixels (column axis).
    double principalPointY = 0.0;    ///< Principal point Y coordinate, in pixels (row axis).

    friend constexpr bool operator==(const Intrinsics&, const Intrinsics&) = default;
};

} // namespace ptiff
