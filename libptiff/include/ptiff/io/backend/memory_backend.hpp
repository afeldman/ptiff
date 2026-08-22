#pragma once

#include <span>

#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io::backend {

/// In-memory ("PMEM") format backend (no file I/O -- scenes held entirely in a memory buffer).
/// Registered with BackendFactory under the name "memory". Persists a
/// @ref ptiff::io::StorageModel "StorageModel" tree -- one child per image -- plus raw,
/// uncompressed pixel tiles as a contiguous region after a self-describing 12-byte header, using
/// the public @ref ptiff::io::MemoryBinaryReader "MemoryBinaryReader" /
/// @ref ptiff::io::MemoryBinaryWriter "MemoryBinaryWriter" for in-memory transport. Supports
/// multiple images per document via @ref serializeModelList / @ref openImageSinkAt /
/// @ref openImageSourceAt. Call serializeModelList (or serializeModel for one image) before a
/// pixel-data write: it writes the header+model tree and openImageSinkAt re-derives the same
/// layout from the same models to position per-image pixel-data writes.
class PTIFF_EXPORT MemoryBackend final : public StorageBackend {
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
