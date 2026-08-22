#pragma once

#include <cstdint>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>

namespace ptiff::io::backend::memory {

/// ImageSink over one image's pixel region of a Memory ("PMEM") document. Writes each tile's raw,
/// uncompressed bytes to `writer` at the linear offset its TileLayout implies (seeking as needed),
/// so tiles may be written in any order. The byte count per tile must equal the layout-derived
/// `tileBytes` (edge tiles are written padded to the full tile size, matching the system-wide
/// contract). Not thread-safe, same contract as ImageSink's base class.
class PTIFF_EXPORT MemoryImageSink final : public ImageSink {
public:
    /// @brief Constructs the sink over a binary writer and the image's pixel region.
    ///
    /// @param writer         The underlying binary writer for the memory buffer.
    /// @param info           The resolved @ref ptiff::io::backend::memory::MemoryImageInfo
    ///                       "MemoryImageInfo" for this image.
    /// @param imagePixelStart Absolute byte offset where this image's pixel region begins.
    MemoryImageSink(BinaryWriter& writer, MemoryImageInfo info, std::uint64_t imagePixelStart);

    /// @brief Returns the tile layout of this image.
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Writes one tile's raw uncompressed bytes at its layout-derived offset.
    ///
    /// MAY be called in any order (offsets are sought as needed). Edge tiles are written padded
    /// to the full tile byte count.
    ///
    /// @param tile The tile whose bytes are written.
    /// @return `Result<void>` on success, or an error if the write fails.
    [[nodiscard]] Result<void> writeTile(const io::tile::Tile& tile) override;

private:
    BinaryWriter& writer_;
    MemoryImageInfo info_;
    std::uint64_t imagePixelStart_;
};

} // namespace ptiff::io::backend::memory
