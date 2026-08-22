#pragma once

#include <array>
#include <memory>
#include <string>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/geometry/extrinsics.hpp>
#include <ptiff/geometry/intrinsics.hpp>

namespace ptiff {

/// A 3x4 projection matrix row-major (`P = K * [R | t]`), 12 doubles.
using ProjectionMatrix = std::array<double, 12>;

/// A 3x3 rotation matrix (row-major) derived from an [`Extrinsics`] rotation quaternion.
using RotationMatrix = std::array<double, 9>;

/// @brief A camera model capturing intrinsic and extrinsic parameters for a single sensor.
///
/// A `Camera` represents the acquisition geometry of one sensor: its intrinsic parameters
/// (@ref ptiff::Intrinsics "Intrinsics") and its extrinsic pose
/// (@ref ptiff::Extrinsics "Extrinsics"), together with an observation timestamp. From these it
/// can synthesise the full 3x4 projection matrix `P = K * [R | t]`, which is the standard
/// pinhole mapping from world to image coordinates.
///
/// The timestamp is an ISO-8601 UTC string (e.g. `"2026-08-21T12:34:56.000Z"`). Time-dependent
/// calibration (a satellite-mounted camera that moves during acquisition) is represented by
/// treating each frame's `Camera` as the pose valid at that frame's timestamp.
class PTIFF_EXPORT Camera {
public:
    /// @brief Constructs a default (identity) camera: model `"pinhole"`, zero intrinsics,
    ///        identity pose, no timestamp.
    Camera();

    /// @brief Constructs a fully-specified camera.
    ///
    /// @param model       Camera model identifier (e.g. `"pinhole"`).
    /// @param intrinsics  Intrinsic parameters (fx, fy, cx, cy).
    /// @param extrinsics  Extrinsic pose (rotations + translation).
    /// @param timestamp   ISO-8601 UTC observation timestamp; empty means unset.
    Camera(std::string model,
           Intrinsics intrinsics,
           Extrinsics extrinsics,
           std::string timestamp = {});

    ~Camera();

    Camera(const Camera&) = delete;
    Camera& operator=(const Camera&) = delete;
    Camera(Camera&&) noexcept;
    Camera& operator=(Camera&&) noexcept;

    /// @brief Returns the camera model identifier (e.g. `"pinhole"`).
    [[nodiscard]] const std::string& modelName() const noexcept;

    /// @brief Returns the intrinsic parameters.
    [[nodiscard]] const Intrinsics& intrinsics() const noexcept;

    /// @brief Returns the extrinsic pose.
    [[nodiscard]] const Extrinsics& extrinsics() const noexcept;

    /// @brief Returns the ISO-8601 UTC observation timestamp (empty when unset).
    [[nodiscard]] const std::string& timestamp() const noexcept;

    /// @brief Builds the 3x3 intrinsic (calibration) matrix K (row-major).
    ///
    ///   K = [ fx  0  cx ]
    ///       [  0 fy  cy ]
    ///       [  0  0   1 ]
    [[nodiscard]] std::array<double, 9> intrinsicsMatrix() const noexcept;

    /// @brief Builds the 3x4 extrinsic matrix `[R | t]` (row-major) from the rotation
    ///        quaternion and the world translation.
    ///
    /// `R` is the camera-to-world rotation; `t` is the camera position in world coordinates.
    [[nodiscard]] std::array<double, 12> extrinsicsMatrix() const noexcept;

    /// @brief Computes the full 3x4 projection matrix `P = K * [R | t]` (row-major).
    ///
    /// This maps a world point `X` to a homogeneous image point via `x ~ P * X`. The result is
    /// the standard pinhole projection built from the two calibration matrices.
    [[nodiscard]] ProjectionMatrix projectionMatrix() const noexcept;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
