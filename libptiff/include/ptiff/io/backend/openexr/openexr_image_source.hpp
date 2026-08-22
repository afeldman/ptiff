#pragma once

// ImageSource over one OpenEXR document's pixels. The backend only supports a single whole-image
// tile this phase (see openexr_document.hpp), so the layout always has exactly one tile; readTile
// lazily decodes the full image through a real Imf::InputFile on first (and only) call and caches
// the interleaved result for the source's lifetime.

#include <cstddef>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/openexr/openexr_document.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/image_source.hpp>

namespace ptiff::io::backend::openexr {

/// @brief ImageSource adapting @ref ptiff::io::ImageSource "ImageSource" read operations to one
///        OpenEXR document's pixel chunks.
///
/// The layout always contains exactly one whole-image tile; `readTile` lazily decodes the full
/// image through a real `Imf::InputFile` on the first (and only) call and caches the interleaved
/// result for the source's lifetime.
class PTIFF_EXPORT OpenExrImageSource final : public ImageSource {
public:
    /// @brief Constructs the source over a binary reader and the resolved image info.
    ///
    /// @param reader The underlying binary reader to stream the .exr document from.
    /// @param info   The resolved @ref ptiff::io::backend::openexr::OpenExrImageInfo
    ///               "OpenExrImageInfo" for this image.
    OpenExrImageSource(io::BinaryReader& reader, OpenExrImageInfo info);

    /// @brief Returns the tile layout of this image (a single whole-image tile).
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Reads the single whole-image tile (decoding the full image on first access).
    ///
    /// @param index The tile index; must address the only tile of the layout.
    /// @return The decoded @ref ptiff::io::tile::Tile "Tile" on success, or an error otherwise.
    [[nodiscard]] Result<io::tile::Tile> readTile(const io::tile::TileIndex& index) override;

private:
    [[nodiscard]] Result<void> ensureLoaded();

    io::BinaryReader& reader_;
    OpenExrImageInfo info_;
    io::tile::TileLayout layout_;
    std::vector<std::byte> buffer_;
    bool loaded_ = false;
};

} // namespace ptiff::io::backend::openexr
