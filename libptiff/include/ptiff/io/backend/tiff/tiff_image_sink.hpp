#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>

namespace ptiff::io::backend::tiff {

/// ImageSink over one TiffDirectory's baseline strip layout. Writes each tile's raw bytes
/// directly to `writer` at the offset TiffBackend's write path (planTiffWrite) already computed
/// -- no compression, no predictor (this backend's write scope). Not thread-safe, same contract
/// as ImageSink's base class.
class PTIFF_EXPORT TiffImageSink final : public ImageSink {
public:
    /// @brief Constructs the sink over a binary writer and the resolved directory.
    ///
    /// @param writer    The underlying binary writer to stream strip bytes through.
    /// @param directory The resolved @ref ptiff::io::backend::tiff::TiffDirectory
    ///                  "TiffDirectory" describing the strip layout to write.
    TiffImageSink(BinaryWriter& writer, TiffDirectory directory);

    /// @brief Returns the strip/tile layout of this image.
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Writes one tile/strip's raw bytes at the pre-computed file offset.
    ///
    /// No compression or prediction is applied (this backend's write scope).
    ///
    /// @param tile The tile whose raw bytes are written.
    /// @return `Result<void>` on success, or an error if the write fails.
    [[nodiscard]] Result<void> writeTile(const io::tile::Tile& tile) override;

private:
    BinaryWriter& writer_;
    TiffDirectory directory_;
};

} // namespace ptiff::io::backend::tiff
