#pragma once

#include <span>

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// TIFF/BigTIFF format backend. Registered with BackendFactory under the name "tiff". Reads
/// baseline TIFF/BigTIFF (see the 2026-07-23 design spec's Format Coverage section for the
/// supported read subset) -- including multi-image files via the IFD chain. Writes baseline
/// classic TIFF / BigTIFF for one or more images: a single image is a single IFD followed by its
/// data; several images are an IFD chain (one directory per image) followed by each image's data
/// region (see the 2026-08-04 multi-image design spec). Call serializeModelList (or serializeModel
/// for one image) before a pixel-data write: it writes the header/IFD chain and openImageSinkAt
/// re-derives the same layout from the same models to position per-image pixel-data writes.
class PTIFF_EXPORT TiffBackend final : public StorageBackend {
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
    [[nodiscard]] Result<void> serializeModelList(std::span<const StorageModel> models,
                                                  BinaryWriter& writer) const override;
    [[nodiscard]] Result<std::unique_ptr<ImageSink>>
    openImageSinkAt(BinaryWriter& writer,
                    std::span<const StorageModel> models,
                    std::size_t imageIndex) const override;
    [[nodiscard]] Result<std::unique_ptr<ImageSource>>
    openImageSourceAt(BinaryReader& reader, std::size_t imageIndex) const override;
};

} // namespace ptiff::io::backend
