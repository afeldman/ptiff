#include <array>
#include <memory>
#include <string>
#include <utility>

#include <ptiff/geometry/camera.hpp>

namespace ptiff {

struct Camera::Impl {
    std::string model;
    Intrinsics intrinsics;
    Extrinsics extrinsics;
    std::string timestamp;
};

Camera::Camera() : impl_(std::make_unique<Impl>()) {
    impl_->model = "pinhole";
}

Camera::Camera(std::string model,
               Intrinsics intrinsics,
               Extrinsics extrinsics,
               std::string timestamp)
    : impl_(std::make_unique<Impl>()) {
    impl_->model = std::move(model);
    impl_->intrinsics = intrinsics;
    impl_->extrinsics = extrinsics;
    impl_->timestamp = std::move(timestamp);
}

Camera::~Camera() = default;
Camera::Camera(Camera&&) noexcept = default;
Camera& Camera::operator=(Camera&&) noexcept = default;

const std::string& Camera::modelName() const noexcept {
    return impl_->model;
}

const Intrinsics& Camera::intrinsics() const noexcept {
    return impl_->intrinsics;
}

const Extrinsics& Camera::extrinsics() const noexcept {
    return impl_->extrinsics;
}

const std::string& Camera::timestamp() const noexcept {
    return impl_->timestamp;
}

std::array<double, 9> Camera::intrinsicsMatrix() const noexcept {
    const auto& i = impl_->intrinsics;
    return {
        i.focalLengthPixelsX,
        0.0,
        i.principalPointX,
        0.0,
        i.focalLengthPixelsY,
        i.principalPointY,
        0.0,
        0.0,
        1.0,
    };
}

std::array<double, 12> Camera::extrinsicsMatrix() const noexcept {
    // Convert the camera-to-world rotation quaternion (w, x, y, z) to a 3x3
    // rotation matrix (row-major), then append the world translation as the
    // fourth column -- yielding the 3x4 [R | t] extrinsic matrix.
    const auto& q = impl_->extrinsics.rotation;
    const auto& t = impl_->extrinsics.translation;

    const double w = q.w, x = q.x, y = q.y, z = q.z;
    const double xx = x * x, yy = y * y, zz = z * z;
    const double xy = x * y, xz = x * z, yz = y * z;
    const double wx = w * x, wy = w * y, wz = w * z;

    return {
        // Row 0
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy - wz),
        2.0 * (xz + wy),
        t.x,
        // Row 1
        2.0 * (xy + wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz - wx),
        t.y,
        // Row 2
        2.0 * (xz - wy),
        2.0 * (yz + wx),
        1.0 - 2.0 * (xx + yy),
        t.z,
    };
}

ProjectionMatrix Camera::projectionMatrix() const noexcept {
    // P = K * [R | t]. K is 3x3 (fx, fy, cx, cy), [R|t] is 3x4.
    const auto& i = impl_->intrinsics;
    const auto& q = impl_->extrinsics.rotation;
    const auto& tr = impl_->extrinsics.translation;

    const double fx = i.focalLengthPixelsX, fy = i.focalLengthPixelsY;
    const double cx = i.principalPointX, cy = i.principalPointY;

    const double w = q.w, x = q.x, y = q.y, z = q.z;
    const double xx = x * x, yy = y * y, zz = z * z;
    const double xy = x * y, xz = x * z, yz = y * z;
    const double wx = w * x, wy = w * y, wz = w * z;

    // Rotation rows of the camera-to-world rotation.
    const std::array<double, 9> R = {
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy - wz),
        2.0 * (xz + wy),
        2.0 * (xy + wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz - wx),
        2.0 * (xz - wy),
        2.0 * (yz + wx),
        1.0 - 2.0 * (xx + yy),
    };
    // The 3x4 extrinsic matrix [R | t], row-major.
    const std::array<double, 12> Rt = {
        R[0],
        R[1],
        R[2],
        tr.x,
        R[3],
        R[4],
        R[5],
        tr.y,
        R[6],
        R[7],
        R[8],
        tr.z,
    };

    // K rows (sparse pinhole calibration matrix).
    const std::array<double, 9> K = {
        fx,
        0.0,
        cx,
        0.0,
        fy,
        cy,
        0.0,
        0.0,
        1.0,
    };

    // P = K * Rt:  P[i][j] = sum_k K[i][k] * Rt[k][j].
    ProjectionMatrix P{};
    for (std::size_t row = 0; row < 3; ++row) {
        for (std::size_t col = 0; col < 4; ++col) {
            double v = 0.0;
            for (std::size_t k = 0; k < 3; ++k) {
                v += K[row * 3 + k] * Rt[k * 4 + col];
            }
            P[row * 4 + col] = v;
        }
    }

    return P;
}

} // namespace ptiff
