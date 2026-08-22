#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/deserializer.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io {

/// @brief Converts a @ref ptiff::io::StorageModel "StorageModel" into a @ref ptiff::Scene "Scene".
///
/// The concrete storage-to-scene adapter, inverse of @ref ptiff::io::SceneSerializer
/// "SceneSerializer". Each child node of the root model becomes one image in the scene, in
/// child order. Accepts compression spellings `"None"`, `"LZW"` and `"Lzw"` (the write path
/// emits `"LZW"`; the read path emits `"Lzw"`).
///
/// @section scene_deserializer_note Round-trip limitation
/// The TIFF storage model does not carry `tileInfo` or `groundSampleDistanceMeters`, so images
/// deserialized from a backend-produced model have neither option set. Only fields a real
/// `toStorageModel` emitted are reconstructed.
///
/// @section scene_deserializer_thread Thread-safety
/// Stateless; thread-compatible.
///
/// @section scene_deserializer_example Example
///
/// @code{.cpp}
/// using ptiff::io::SceneDeserializer;
/// using ptiff::io::StorageModel;
///
/// StorageModel root;
/// StorageModel image;
/// image.setField("imageWidth", "64");
/// image.setField("imageHeight", "32");
/// image.setField("samplesPerPixel", "1");
/// image.setField("pixelType", "UInt8");
/// root.addChild(std::move(image));
///
/// SceneDeserializer deser;
/// auto scene = deser.deserialize(root);
/// assert(scene.has_value() && *scene->imageCount() == 1);
/// @endcode
///
/// @see @ref ptiff::io::SceneSerializer "SceneSerializer" (inverse).
class PTIFF_EXPORT SceneDeserializer final : public Deserializer {
public:
    SceneDeserializer() = default;

    /// @brief Converts \p model into a domain-model scene.
    /// @param model The format-neutral storage model to deserialize.
    /// @return The resulting @ref ptiff::Scene "Scene" on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if a child node is
    ///         missing a required field or holds an unrecognized pixelType/compression value.
    [[nodiscard]] Result<Scene> deserialize(const StorageModel& model) const override;
};

} // namespace ptiff::io
