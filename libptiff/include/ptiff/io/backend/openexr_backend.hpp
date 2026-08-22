#pragma once

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// OpenEXR HDR image format backend. Registered with BackendFactory under the name "openexr".
/// Unlike PDS4/ISIS (a hand-rolled text/XML label wrapping a raw pixel block), an OpenExrBackend
/// document is a real, standalone .exr file, read/written entirely through the genuine Imf::
/// API (see openexr/openexr_document.hpp). Only Float32 and UInt32 pixel types are supported
/// (the two that map 1:1 onto a native OpenEXR channel type), with samplesPerPixel 1 ("Y"), 3
/// ("R","G","B") or 4 ("R","G","B","A"); the whole image is always one tile (genuine multi-tile
/// OpenEXR chunking is deferred). Serializes/deserializes a single image (root with one child in
/// the Scene convention); the multi-image `serializeModelList` / `openImageSinkAt` /
/// `openImageSourceAt` entry points remain `NotImplemented`.
class PTIFF_EXPORT OpenExrBackend final : public StorageBackend {
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
