#pragma once

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// Zarr-style chunked array backend. Registered with BackendFactory under the name "zarr".
/// Persists a format-neutral image as a compact self-describing single-file container: an 8-byte
/// seek header, a JSON array header (shape/chunks/dtype/compressor via nlohmann_json), then
/// fixed-size chunk slots holding zstd/zlib-compressed (or raw) chunks (chunk == tile for this
/// phase). Serializes a single image (root with one child in the Scene convention); the
/// multi-image `serializeModelList` / `openImageSinkAt` / `openImageSourceAt` entry points remain
/// `NotImplemented`.
class PTIFF_EXPORT ZarrBackend final : public StorageBackend {
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
