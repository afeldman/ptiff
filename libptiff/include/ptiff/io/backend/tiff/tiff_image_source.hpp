#pragma once

#include <cstddef>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/image_source.hpp>

namespace ptiff::io::backend::tiff {

/// ImageSource over one TIFF/BigTIFF IFD's baseline image. Reads each tile/strip's raw bytes
/// directly from `reader` on demand, decodes PackBits/LZW compression if present, then undoes
/// Predictor=2 horizontal differencing if present -- no upfront full-image read. The expected
/// decoded size is computed uniformly as tileWidth*tileHeight*samplesPerPixel*bytesPerSample;
/// this is exact for tiled layouts (TIFF pads edge tiles to full size) and for stripped layouts
/// where RowsPerStrip evenly divides ImageLength -- a stripped layout with a short last strip
/// combined with compression is a known, accepted gap (fails with InvalidArgument rather than
/// silently misreading). readTile()'s returned Tile view is valid until the next readTile() call
/// or until this TiffImageSource is destroyed, whichever comes first -- it views
/// TiffImageSource's own reusable buffer, the same invalidation contract as a single-buffer
/// iterator.
class PTIFF_EXPORT TiffImageSource final : public ImageSource {
public:
    TiffImageSource(BinaryReader& reader, TiffDirectory directory);

    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;
    [[nodiscard]] Result<io::tile::Tile> readTile(const io::tile::TileIndex& index) override;

private:
    BinaryReader& reader_;
    TiffDirectory directory_;
    std::vector<std::byte> rawBuffer_;
    std::vector<std::byte> buffer_;
};

} // namespace ptiff::io::backend::tiff
