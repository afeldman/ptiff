#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

namespace ptiff::io {

/// @brief Converts a @ref ptiff::Scene "Scene" into a format-neutral
///        @ref ptiff::io::StorageModel "StorageModel".
///
/// A Serializer is the boundary between the **domain model** (a @ref ptiff::Scene "Scene", above
/// this layer) and the **storage layer** (a @ref ptiff::io::StorageModel "StorageModel", below).
/// It maps the scene's semantic content -- images, cameras, layers, annotations -- into the
/// structured, format-neutral nodes and fields of a StorageModel, without touching any bytes.
///
/// @section serializer_boundary No bytes cross here
///
/// No binary data crosses this boundary. Everything below `StorageModel` is a
/// @ref ptiff::io::StorageBackend "StorageBackend"'s problem; everything above it is the domain
/// model's. Keeping the two sides decoupled is what lets the same scene serialize to any format.
///
/// @section serializer_thread Thread-safety
///
/// Thread-compatible if stateless, which is the expected case (a pure function of its argument).
/// A caching implementation must document otherwise.
///
/// @section serializer_example Example
///
/// @code{.cpp}
/// using ptiff::io::Serializer;
/// using ptiff::io::StorageModel;
///
/// // `ser` is a concrete Serializer; `scene` a ptiff::Scene to persist.
/// auto model = ser.serialize(scene);
/// assert(model.has_value());
/// @endcode
///
/// @see @ref ptiff::io::Deserializer "Deserializer" (inverse),
///      @ref ptiff::io::StorageModel "StorageModel",
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT Serializer {
public:
    virtual ~Serializer() = default;
    Serializer(const Serializer&) = delete;
    Serializer& operator=(const Serializer&) = delete;
    Serializer(Serializer&&) = delete;
    Serializer& operator=(Serializer&&) = delete;

    /// @brief Converts \p scene into a format-neutral storage model.
    ///
    /// @param scene The domain-model scene to serialize.
    /// @return The resulting @ref ptiff::io::StorageModel "StorageModel" on success, or a
    ///         serializer-specific error (typically
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument") if \p scene cannot be
    ///         represented in the format-neutral model.
    [[nodiscard]] virtual Result<StorageModel> serialize(const Scene& scene) const = 0;

protected:
    Serializer() = default;
};

} // namespace ptiff::io
