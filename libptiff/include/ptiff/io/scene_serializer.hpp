
#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/serializer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

namespace ptiff::io {

/// @brief Converts a @ref ptiff::Scene "Scene" into a @ref ptiff::io::StorageModel "StorageModel".
///
/// The concrete Scene-to-storage adapter. Each image in the scene becomes one child node of a
/// root model, in scene order. Field names and value spellings match the TIFF backend's
/// `planTiffWrite` contract exactly, so any model the Serializer emits can be written by
/// @ref ptiff::io::StorageBackend "StorageBackend".
///
/// @section scene_serializer_mapping Mapping
/// `width`/`height` -> `"imageWidth"`/`"imageHeight"`, `channelCount` -> `"samplesPerPixel"`
/// (only 1 or 3), `pixelType` -> `"pixelType"` (`Float64` is rejected -> InvalidArgument),
/// `compression` -> `"compression"` (`None` -> `"None"`, `Lzw` -> `"LZW"`; `Deflate`/`Jpeg`
/// rejected), `tileInfo` -> `"tileWidth"`/`"tileHeight"`.
///
/// @section scene_serializer_note Round-trip limitation
/// `groundSampleDistanceMeters` is not represented in the TIFF storage model and is dropped on
/// serialize.
///
/// @section scene_serializer_thread Thread-safety
/// Stateless; thread-compatible (a pure function of its argument).
///
/// @section scene_serializer_example Example
///
/// @code{.cpp}
/// using ptiff::io::SceneSerializer;
///
/// ptiff::Scene scene;
/// scene.addImage(ptiff::ImageDescriptor{});
/// SceneSerializer ser;
/// auto model = ser.serialize(scene);
/// assert(model.has_value());
/// assert(model->children().size() == 1);
/// @endcode
///
/// @see @ref ptiff::io::SceneDeserializer "SceneDeserializer" (inverse).
class PTIFF_EXPORT SceneSerializer final : public Serializer {
public:
    SceneSerializer() = default;

    /// @brief Converts \p scene into a storage model.
    /// @param scene The domain-model scene to serialize.
    /// @return The resulting @ref ptiff::io::StorageModel "StorageModel" on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if any image's
    ///         pixelType is Float64, compression is Deflate/Jpeg, or channelCount is neither 1
    ///         nor 3.
    [[nodiscard]] Result<StorageModel> serialize(const Scene& scene) const override;
};

} // namespace ptiff::io
