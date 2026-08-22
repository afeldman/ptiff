#pragma once

#include <memory>
#include <string>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief A derived per-pixel data layer aligned to an image's grid (e.g. depth, segmentation).
///
/// A `ScientificLayer` represents an additional raster aligned to an owning image's pixel grid,
/// carrying a derived quantity (see @ref ptiff::LayerKind "LayerKind": depth, surface normals,
/// temperature, mask, ...).
///
/// Sprint-1 note: this is an interface-only stub. Every accessor returns
/// @ref ptiff::ErrorCode::NotImplemented "ErrorCode::NotImplemented" until layers exist.
/// Move-only; copy semantics will be decided once there is real state.
class PTIFF_EXPORT ScientificLayer {
public:
    ScientificLayer();
    ~ScientificLayer();

    ScientificLayer(const ScientificLayer&) = delete;
    ScientificLayer& operator=(const ScientificLayer&) = delete;
    ScientificLayer(ScientificLayer&&) noexcept;
    ScientificLayer& operator=(ScientificLayer&&) noexcept;

    /// @brief Returns the layer name (e.g. `"depth"`, `"segmentation"`).
    ///
    /// Sprint-1 stub: returns @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @return The layer's name on success, or an error otherwise.
    [[nodiscard]] Result<std::string> name() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
