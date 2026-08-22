#pragma once

#include <memory>
#include <optional>
#include <string>
#include <string_view>

#include <ptiff/core/id.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief What an @ref ptiff::Annotation "Annotation" contains.
///
/// Reserved extension point for annotations (bounding boxes, polygons, segmentation, named
/// features such as a crater or landing site). No concrete kind exists yet beyond
/// `Unspecified`.
enum class AnnotationKind {
    Unspecified, ///< No specific kind assigned.
};

/// @brief An annotation tied to a scene (bounding box, polygon, named feature, ...).
///
/// `Annotation` is the extension point for labeled/derived image features (bounding boxes,
/// polygons, segmentation masks, named craters/landing sites, ...). It currently carries a kind
/// and an optional source image, plus an unstructured named-parameter store.
///
/// There is no `id()` accessor -- Annotation identity is scoped to whichever
/// @ref ptiff::Scene "Scene" it was added to; see Scene::addAnnotation().
///
/// @section annotation_example Example
///
/// @code{.cpp}
/// using ptiff::Annotation;
/// using ptiff::AnnotationKind;
///
/// Annotation a(AnnotationKind::Unspecified);
/// a.setParameter("name", "Tsiolkovskiy");
///
/// auto name = a.parameter("name");
/// if (name) assert(*name == "Tsiolkovskiy");
/// @endcode
class PTIFF_EXPORT Annotation {
public:
    /// @brief Constructs an annotation.
    ///
    /// @param kind        The @ref ptiff::AnnotationKind "kind" of this annotation.
    /// @param sourceImage The optional id of the image this annotation refers to.
    explicit Annotation(AnnotationKind kind, std::optional<ImageId> sourceImage = std::nullopt);
    ~Annotation();

    Annotation(const Annotation&) = delete;
    Annotation& operator=(const Annotation&) = delete;
    Annotation(Annotation&&) noexcept;
    Annotation& operator=(Annotation&&) noexcept;

    /// @brief Returns the annotation kind.
    [[nodiscard]] AnnotationKind kind() const noexcept;

    /// @brief Returns the optional id of the source image this annotation refers to.
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
