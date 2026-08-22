#pragma once

#include <memory>
#include <optional>
#include <string>
#include <string_view>

#include <ptiff/core/id.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief What a @ref ptiff::Geometry "Geometry" represents.
///
/// Identifies the class of 3D geometric product carried by a geometry object. No concrete kind
/// exists yet beyond `Unspecified` -- a real (if empty) type rather than a reserved namespace,
/// so the enum can grow concrete domain values without breaking callers.
enum class GeometryKind {
    Unspecified, ///< No specific kind assigned.
};

/// @brief A 3D geometric product associated with a scene (mesh, point cloud, parametric
///        surface), reserved for future extension.
///
/// A `Geometry` is the extension point for 3D geometric data tied to a @ref ptiff::Scene
/// "Scene" (e.g. a mesh produced by stereo processing). It currently carries a kind and an
/// optional source image, plus an unstructured named-parameter store.
///
/// There is no `id()` accessor -- Geometry identity is scoped to whichever
/// @ref ptiff::Scene "Scene" it was added to; see Scene::addGeometry().
///
/// @section geometry_example Example
///
/// @code{.cpp}
/// using ptiff::Geometry;
/// using ptiff::GeometryKind;
///
/// Geometry g(GeometryKind::Unspecified);
/// g.setParameter("units", "meters");
///
/// auto units = g.parameter("units");
/// if (units) assert(*units == "meters");
/// @endcode
class PTIFF_EXPORT Geometry {
public:
    /// @brief Constructs a geometry product.
    ///
    /// @param kind        The @ref ptiff::GeometryKind "kind" of this geometry.
    /// @param sourceImage The optional id of the image this geometry derives from.
    explicit Geometry(GeometryKind kind, std::optional<ImageId> sourceImage = std::nullopt);
    ~Geometry();

    Geometry(const Geometry&) = delete;
    Geometry& operator=(const Geometry&) = delete;
    Geometry(Geometry&&) noexcept;
    Geometry& operator=(Geometry&&) noexcept;

    /// @brief Returns the geometry kind.
    [[nodiscard]] GeometryKind kind() const noexcept;

    /// @brief Returns the optional id of the source image this geometry derives from.
    ///
    /// @return `std::nullopt` if no source image was associated.
    [[nodiscard]] std::optional<ImageId> sourceImage() const noexcept;

    /// @brief Looks up a named parameter.
    ///
    /// @param key The parameter name.
    /// @return The parameter value on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p key was never set.
    [[nodiscard]] Result<std::string> parameter(std::string_view key) const;

    /// @brief Sets a named parameter.
    ///
    /// @param key   The parameter name.
    /// @param value The parameter value (string).
    void setParameter(std::string_view key, std::string value);

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
