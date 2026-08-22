#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/image_source.hpp>

namespace ptiff::io::backend::memory {

/// ImageSource over one image's pixel region of a Memory ("PMEM") document. Reads each tile's
/// raw, uncompressed bytes directly from `reader` on demand, at the linear offset computed from
/// the image's TileLayout -- no upfront full-image read. readTile()'s returned Tile view is
/// valid until the next readTile() call or until this MemoryImageSource is destroyed,
/// whichever comes first -- it views the source's own reusable buffer, the same invalidation
/// contract as a single-buffer iterator. Not thread-safe, same contract as ImageSource's base.
class PTIFF_EXPORT MemoryImageSource final : public ImageSource {
public:
    /// @brief Constructs the source over a binary reader and the image's pixel region.
    ///
    /// @param reader          The underlying binary reader for the memory buffer.
    /// @param info            The resolved @ref ptiff::io::backend::memory::MemoryImageInfo
    ///                        "MemoryImageInfo" for this image.
    /// @param imagePixelStart Absolute byte offset where this image's pixel region begins.
    MemoryImageSource(BinaryReader& reader, MemoryImageInfo info, std::uint64_t imagePixelStart);

    /// @brief Returns the tile layout of this image.
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Reads one tile's raw uncompressed bytes on demand.
    ///
    /// The returned `Tile` view is valid until the next `readTile` call or until this source is
    /// destroyed, whichever comes first (the source owns its reusable single buffer).
    ///
    /// @param index The index of the tile to read.
    /// @return A `Tile` view over the tile's bytes on success, or an error otherwise.
    [[nodiscard]] Result<io::tile::Tile> readTile(const io::tile::TileIndex& index) override;

private:
    BinaryReader& reader_;
    MemoryImageInfo info_;
    std::uint64_t imagePixelStart_;
    std::vector<std::byte> buffer_;
};

} // namespace ptiff::io::backend::memory
