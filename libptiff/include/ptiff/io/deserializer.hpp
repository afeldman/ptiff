#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

namespace ptiff::io {

/// @brief Converts a format-neutral @ref ptiff::io::StorageModel "StorageModel" into a
///        @ref ptiff::Scene "Scene".
///
/// Deserializer is the inverse of @ref ptiff::io::Serializer "Serializer": it maps the
/// structured, format-neutral nodes and fields of a StorageModel back into the domain model's
/// @ref ptiff::Scene "Scene" -- images, cameras, layers, annotations. As with its mirror, no
/// binary data crosses this boundary; turning bytes into a StorageModel is a
/// @ref ptiff::io::StorageBackend "StorageBackend" concern.
///
/// @section deserializer_thread Thread-safety
///
/// Mirrors @ref ptiff::io::Serializer "Serializer": thread-compatible if stateless (the expected
/// case); a caching implementation must document otherwise.
///
/// @section deserializer_example Example
///
/// @code{.cpp}
/// using ptiff::io::Deserializer;
///
/// // `deser` is a concrete Deserializer; `model` a StorageModel read from some backend.
/// auto scene = deser.deserialize(model);
/// assert(scene.has_value());
/// @endcode
///
/// @see @ref ptiff::io::Serializer "Serializer" (inverse),
///      @ref ptiff::io::StorageModel "StorageModel",
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT Deserializer {
public:
    virtual ~Deserializer() = default;
    Deserializer(const Deserializer&) = delete;
    Deserializer& operator=(const Deserializer&) = delete;
    Deserializer(Deserializer&&) = delete;
    Deserializer& operator=(Deserializer&&) = delete;

    /// @brief Converts \p model into a domain-model scene.
    ///
    /// @param model The format-neutral storage model to deserialize.
    /// @return The resulting @ref ptiff::Scene "Scene" on success, or a deserializer-specific
    ///         error (typically @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument") if
    ///         \p model is missing fields or contradicts the domain model's constraints.
    [[nodiscard]] virtual Result<Scene> deserialize(const StorageModel& model) const = 0;

protected:
    Deserializer() = default;
};

} // namespace ptiff::io
