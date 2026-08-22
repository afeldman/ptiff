#pragma once

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// NASA PDS4 (Planetary Data System 4) style backend. Registered with BackendFactory under the
/// name "pds4". Persists a format-neutral image as a small self-describing PDS4-looking subset:
/// a `Product_Observational` XML label (written/read via pugixml) followed by a seekable,
/// uncompressed pixel block. Serializes/deserializes a single image (root with one child in the
/// Scene convention); the multi-image `serializeModelList` / `openImageSinkAt` /
/// `openImageSourceAt` entry points remain `NotImplemented` and will gain N>1 support in a later
/// phase.
class PTIFF_EXPORT Pds4Backend final : public StorageBackend {
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
