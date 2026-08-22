#pragma once

// ImageSink over one OpenEXR document's pixels. The backend only supports a single whole-image
// tile this phase (see openexr_document.hpp), so writeTile() is expected exactly once, with the
// tile's data spanning the whole image; it owns one complete Imf::OutputFile write session
// (header + all pixel chunks) for that single call, always starting at writer offset 0 -- this
// intentionally supersedes/overwrites whatever header-only bytes serializeModel() may already
// have written there (see openexr_document.hpp's writeHeaderOnly), since OpenEXR's own chunk
// structure cannot be split across two independent OutputFile sessions.

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/openexr/openexr_document.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>

namespace ptiff::io::backend::openexr {

/// @brief ImageSink adapting @ref ptiff::io::ImageSink "ImageSink" write operations to one
///        OpenEXR document's pixel chunks.
///
/// Owns one `Imf::OutputFile` session used by a single `writeTile` call covering the whole
/// image. See the file-level comment above for the one-tile writesession model and its
/// interplay with `serializeModel`/`writeHeaderOnly`.
class PTIFF_EXPORT OpenExrImageSink final : public ImageSink {
public:
    /// @brief Constructs the sink over a binary writer and the resolved image info.
    ///
    /// @param writer The underlying binary writer to stream the .exr document through.
    /// @param info   The resolved @ref ptiff::io::backend::openexr::OpenExrImageInfo
    ///               "OpenExrImageInfo" for this image.
    OpenExrImageSink(io::BinaryWriter& writer, OpenExrImageInfo info);

    /// @brief Returns the tile layout of this image (a single whole-image tile).
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Writes one tile (the whole image) as the dedicated pixel chunk.
    ///
    /// Must be called exactly once for the single whole-image tile.
    ///
    /// @param tile The tile whose pixel data spans the whole image.
    /// @return `Result<void>` on success, or an error if the write fails.
    [[nodiscard]] Result<void> writeTile(const io::tile::Tile& tile) override;

private:
    io::BinaryWriter& writer_;
    OpenExrImageInfo info_;
    io::tile::TileLayout layout_;
};

} // namespace ptiff::io::backend::openexr
