#pragma once

#include <cstdint>

namespace ptiff {

namespace detail {
/// @cond INTERNAL
struct ImageIdTag {};
struct CameraIdTag {};
struct LayerIdTag {};
struct AnnotationIdTag {};
struct GeometryIdTag {};
struct TileIdTag {};
/// @endcond
} // namespace detail

/// @brief Strongly-typed opaque handle.
///
/// `Id<Tag>` wraps a `std::uint64_t` value but keeps it **distinct from every other Id type** at
/// compile time. The \p Tag template parameter exists only for that purpose: it has no data of
/// its own and is never instantiated by itself -- its sole job is to stop, say,
/// @ref ptiff::ImageId "ImageId" and @ref ptiff::CameraId "CameraId" from being interchanged by
/// accident.
///
/// @section id_lifetime Scoping
///
/// Id values carry **no meaning outside the Scene that issued them**. Two ids from different
/// scenes are not comparable in a meaningful way, and the same numeric value may identify
/// different objects in different scenes. Ids are minted by `Scene::addImage()` and friends, not
/// hand-rolled by callers.
///
/// @section id_example Example
///
/// @code{.cpp}
/// using ptiff::ImageId;
///
/// ImageId image = ImageId{42}; // explicit conversion from the raw value
/// assert(image.value() == 42);
///
/// // Distinct tags prevent accidental mix-ups:
/// //   ptiff::CameraId c{42};
/// //   (void)(image == c);       // COMPILE ERROR: different Id<Tag> types
/// assert(image == ImageId{42}); // comparing equal images is fine
///
/// // Ids are deliberately constrained: no arithmetic (+, -, ...) and no implicit conversion.
/// @endcode
///
/// @tparam Tag An empty tag type (see the `detail` tags below) used only to disambiguate the
///         handle. Naming an Id's tag is what gives it its meaning (ImageId vs TileId, ...).
template <class Tag> class Id {
public:
    /// @brief Wraps a raw 64-bit value into this Id type.
    ///
    /// @param value The opaque handle's numeric value. The conversion is `explicit`, so the
    ///              value only becomes an `Id` where the caller says so.
    constexpr explicit Id(std::uint64_t value) noexcept : value_(value) {}

    /// @brief Returns the underlying numeric value.
    ///
    /// @return The raw `std::uint64_t` this Id wraps.
    [[nodiscard]] constexpr std::uint64_t value() const noexcept { return value_; }

    /// Defaulted value equality; two Ids are equal iff their numeric values are equal.
    friend constexpr bool operator==(const Id&, const Id&) = default;

private:
    std::uint64_t value_;
};

/// Unique id of an @ref ptiff::Image "Image" within its scene.
using ImageId = Id<detail::ImageIdTag>;
/// Unique id of a @ref ptiff::Camera "Camera" within its scene.
using CameraId = Id<detail::CameraIdTag>;
/// Unique id of a @ref ptiff::ScientificLayer "ScientificLayer" within its scene.
using LayerId = Id<detail::LayerIdTag>;
/// Unique id of an @ref ptiff::Annotation "Annotation" within its scene.
using AnnotationId = Id<detail::AnnotationIdTag>;
/// Unique id of a @ref ptiff::Geometry "Geometry" within its scene.
using GeometryId = Id<detail::GeometryIdTag>;
/// Unique id of a tile within a backend (see @ref ptiff::io::tile::Tile "Tile").
using TileId = Id<detail::TileIdTag>;

} // namespace ptiff
