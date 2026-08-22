#pragma once

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// USGS ISIS3-style backend. Registered with BackendFactory under the name "isis". Persists a
/// format-neutral image as a small self-describing ISIS3-looking subset: a PDS3-style text label
/// (Object=IsisCube / Object=Core / Group=Dimensions & Pixels) followed by a seekable,
/// uncompressed pixel block addressed by the label's trailing "End" marker. Serializes a single
/// image (root with one child in the Scene convention); the multi-image
/// `serializeModelList` / `openImageSinkAt` / `openImageSourceAt` entry points remain
/// `NotImplemented` (one image per cube, deferred) .
class PTIFF_EXPORT IsisBackend final : public StorageBackend {
public:
    [[nodiscard]] std::string_view name() const noexcept override;
    [[nodiscard]] BackendCapabilities capabilities() const noexcept override;
    [[nodiscard]] Result<std::unique_ptr<ImageSource>>
    openImageSource(BinaryReader& reader) const override;
    [[nodiscard]] Result<std::unique_ptr<ImageSink>>
    openImageSink(BinaryWriter& writer, const StorageModel& model) const override;
    [[nodiscard]] Result<StorageModel> deserializeModel(BinaryReader& reader) const override;
    [[nodiscard]] Result<void> serializeModel(const StorageModel& model,
                                              BinaryWriter& writer) const override;
};

} // namespace ptiff::io::backend
