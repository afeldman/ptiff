#pragma once

/// @file camera.hpp
/// @brief `ptiff::Camera` — structured camera geometry over the C-ABI camera
///        struct (`ptiff_camera.h`).
///
/// `Camera` carries the pinhole intrinsics (fx/fy/cx/cy + 3x3 matrix), the
/// extrinsics (rotation quaternion + world translation + 3x4 matrix + derived
/// world-to-camera projection), and the ISO-8601 observation timestamp. It is
/// a value type that marshals to/from the C-ABI `struct ptiff_camera` (row-
/// major `double` matrices), so files can round-trip their structured camera
/// domain (`ptiff.camera.*`, tag 65002) through the same C++ API names the
/// reference library exposed.

#include <array>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp>
#include <ptiff/image.hpp>

namespace ptiff {

using ProjectionMatrix = std::array<double, 12>;
using RotationMatrix = std::array<double, 9>;

/// @brief Structured camera value (intrinsics + extrinsics + timestamp).
class Camera {
public:
    Camera() = default;
    Camera(double focalX, double focalY, double cx, double cy,
            double rotW, double rotX, double rotY, double rotZ,
            double posX, double posY, double posZ, std::string ts = {})
        : hasIntrinsics_(true), focalX_(focalX), focalY_(focalY), cx_(cx), cy_(cy),
            hasExtrinsics_(true), rotW_(rotW), rotX_(rotX), rotY_(rotY), rotZ_(rotZ),
            posX_(posX), posY_(posY), posZ_(posZ), timestamp_(std::move(ts)) {
        rebuildMatrices();
    }

    [[nodiscard]] bool hasIntrinsics() const noexcept { return hasIntrinsics_; }
    [[nodiscard]] bool hasExtrinsics() const noexcept { return hasExtrinsics_; }
    [[nodiscard]] double focalLengthX() const noexcept { return focalX_; }
    [[nodiscard]] double focalLengthY() const noexcept { return focalY_; }
    [[nodiscard]] double principalX() const noexcept { return cx_; }
    [[nodiscard]] double principalY() const noexcept { return cy_; }
    [[nodiscard]] double rotationW() const noexcept { return rotW_; }
    [[nodiscard]] double rotationX() const noexcept { return rotX_; }
    [[nodiscard]] double rotationY() const noexcept { return rotY_; }
    [[nodiscard]] double rotationZ() const noexcept { return rotZ_; }
    [[nodiscard]] double positionX() const noexcept { return posX_; }
    [[nodiscard]] double positionY() const noexcept { return posY_; }
    [[nodiscard]] double positionZ() const noexcept { return posZ_; }
    [[nodiscard]] const std::string& timestamp() const noexcept { return timestamp_; }

    /// @brief Reads the structured camera domain from a file (tag 65002).
    static Result<Camera> readFrom(const std::string& path) {
        detail::ptiff_camera c{};
        int32_t rc = detail::ptiff_open_path_camera(path.c_str(), &c);
        if (rc != 0) {
            return std::unexpected(Error{cErrorCodeToEnum(rc),
                                            "Camera::readFrom: open_path_camera failed"});
        }
        if (!c.has_intrinsics && !c.has_extrinsics) {
            return std::unexpected(Error{ErrorCode::NotFound,
                                            "Camera::readFrom: file has no camera domain"});
        }
        Camera cam;
        if (c.has_intrinsics) {
            cam.hasIntrinsics_ = true;
            cam.focalX_ = c.focal_length_x;
            cam.focalY_ = c.focal_length_y;
            cam.cx_ = c.principal_x;
            cam.cy_ = c.principal_y;
        }
        if (c.has_extrinsics) {
            cam.hasExtrinsics_ = true;
            cam.rotW_ = c.rotation_w;
            cam.rotX_ = c.rotation_x;
            cam.rotY_ = c.rotation_y;
            cam.rotZ_ = c.rotation_z;
            cam.posX_ = c.position_x;
            cam.posY_ = c.position_y;
            cam.posZ_ = c.position_z;
        }
        cam.timestamp_ = c.timestamp;
        cam.rebuildMatrices();
        return cam;
    }

    /// @brief Marshals this camera into the C-ABI value struct.
    [[nodiscard]] detail::ptiff_camera toC() const {
        detail::ptiff_camera c{};
        c.has_intrinsics = hasIntrinsics_ ? 1 : 0;
        c.focal_length_x = focalX_;
        c.focal_length_y = focalY_;
        c.principal_x = cx_;
        c.principal_y = cy_;
        c.has_extrinsics = hasExtrinsics_ ? 1 : 0;
        c.rotation_w = rotW_;
        c.rotation_x = rotX_;
        c.rotation_y = rotY_;
        c.rotation_z = rotZ_;
        c.position_x = posX_;
        c.position_y = posY_;
        c.position_z = posZ_;
        for (int i = 0; i < 9; ++i) c.intrinsics[i] = intrinsics_[i];
        for (int i = 0; i < 12; ++i) c.extrinsics[i] = extrinsics_[i];
        for (int i = 0; i < 12; ++i) c.projection[i] = projection_[i];
        std::size_t n = timestamp_.size();
        if (n >= sizeof(c.timestamp)) n = sizeof(c.timestamp) - 1;
        std::memcpy(c.timestamp, timestamp_.data(), n);
        c.timestamp[n] = '\0';
        return c;
    }

    [[nodiscard]] const RotationMatrix& intrinsicsMatrix() const noexcept { return intrinsics_; }
    [[nodiscard]] const std::array<double, 12>& extrinsicsMatrix() const noexcept {
        return extrinsics_;
    }
    [[nodiscard]] const ProjectionMatrix& projectionMatrix() const noexcept { return projection_; }

private:
    void rebuildMatrices() {
        intrinsics_.fill(0.0);
        intrinsics_[0] = focalX_;
        intrinsics_[4] = focalY_;
        intrinsics_[2] = cx_;
        intrinsics_[5] = cy_;
        intrinsics_[8] = 1.0;
        // 3x4: rotation rows then translation column. For a quaternion
        // rotation we store the normalized rotation matrix rows (row-major,
        // 3x3) and the world position as the final column.
        double rot[9]{};
        quatToRotation(rotW_, rotX_, rotY_, rotZ_, rot);
        for (int i = 0; i < 3; ++i) {
            for (int j = 0; j < 3; ++j) extrinsics_[i * 4 + j] = rot[i * 3 + j];
        }
        extrinsics_[3] = posX_;
        extrinsics_[7] = posY_;
        extrinsics_[11] = posZ_;
        std::memcpy(projection_.data(), extrinsics_.data(), 12 * sizeof(double));
    }
    static void quatToRotation(double w, double x, double y, double z, double out[9]) {
        const double m00 = 1 - 2 * (y * y + z * z), m01 = 2 * (x * y - z * w), m02 = 2 * (x * z + y * w);
        const double m10 = 2 * (x * y + z * w), m11 = 1 - 2 * (x * x + z * z), m12 = 2 * (y * z - x * w);
        const double m20 = 2 * (x * z - y * w), m21 = 2 * (y * z + x * w), m22 = 1 - 2 * (x * x + y * y);
        out[0] = m00; out[1] = m01; out[2] = m02;
        out[3] = m10; out[4] = m11; out[5] = m12;
        out[6] = m20; out[7] = m21; out[8] = m22;
    }

    bool hasIntrinsics_ = false;
    double focalX_ = 0.0, focalY_ = 0.0, cx_ = 0.0, cy_ = 0.0;
    bool hasExtrinsics_ = false;
    double rotW_ = 1.0, rotX_ = 0.0, rotY_ = 0.0, rotZ_ = 0.0;
    double posX_ = 0.0, posY_ = 0.0, posZ_ = 0.0;
    std::string timestamp_;
    RotationMatrix intrinsics_{};
    std::array<double, 12> extrinsics_{};
    ProjectionMatrix projection_{};
};

} // namespace ptiff
