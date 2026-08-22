#pragma once

#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::tiff {

/// Everything TiffBackend needs to write one baseline TIFF file: the resolved TiffDirectory
/// (shared with the read path -- TiffImageSink reads offsets from it exactly like
/// TiffImageSource does) and the exact IFD entries to hand to writeTiffIfd.
struct TiffWritePlan {
    TiffDirectory directory;
    std::vector<TiffIfdEntryToWrite> entries;
    bool isBigTiff = false;
};

/// Top-level layout of a whole TIFF file that may hold several images. `isBigTiff` is shared by
/// every image (a file's header declares one container kind); `images` holds one plan per image in
/// file (chain) order.
struct TiffFileWritePlan {
    std::vector<TiffWritePlan> images;
    bool isBigTiff = false;
};

/// Builds a TiffWritePlan from a format-neutral StorageModel. Required fields: "imageWidth",
/// "imageHeight", "samplesPerPixel" (must be 1 or 3), "pixelType" (one of
/// "UInt8"/"UInt16"/"UInt32"/"Float32"). Optional "compression" maps "None" (1), "PackBits"
/// (32773) or "LZW" (5); optional "predictor" maps "None" or "HorizontalDifferencing" (2).
/// Optional "container" selects "Classic" (default) or "BigTiff" -- BigTiff emits a 16-byte
/// BigTIFF header and 20-byte IFD entries instead of the classic 8-byte header and 12-byte
/// entries; unsupported values are Error::InvalidArgument. Horizontal differencing is allowed
/// only for integer pixel types (UInt8/UInt16/UInt32); it is rejected for float samples. When
/// compression is used, the emitted StripByteCounts is a 0 placeholder and
/// directory.stripByteCountsPatchOffset fixes the absolute offset the image sink back-patches
/// with the real compressed byte count after encode.
/// Optional "tileWidth"/"tileHeight" (both must be set together, or both absent) select a tiled
/// layout instead of the default single strip covering the whole image: emits
/// TileWidth/TileLength/TileOffsets/TileByteCounts tags and one TileByteRange per tile
/// (row-major) instead of RowsPerStrip/StripOffsets/StripByteCounts and a single TileByteRange.
/// tileWidth/tileHeight must be nonzero multiples of 16; tiled write does not yet support
/// compression or a predictor (Error::InvalidArgument if either is set to something other than
/// None/absent together with tileWidth/tileHeight). Always little-endian. Error::InvalidArgument
/// for a missing/unparsable required field, unsupported samplesPerPixel/pixelType/compression/
/// predictor/container/tileWidth/tileHeight value, float samples combined with horizontal
/// differencing, or compression/predictor combined with a tiled layout.
[[nodiscard]] Result<TiffWritePlan> planTiffWrite(const StorageModel& model);

/// Builds a TiffFileWritePlan -- the top-level layout of a whole TIFF file that may hold several
/// images (a chain of IFDs). Each image's offsets are stitched contiguously: header, then IFD 0 ..
/// IFD N-1 (each linked to the next through its next-IFD offset; the last's next-IFD is 0), then
/// every image's pixel data. Each image's TiffDirectory.tileByteRanges and
/// stripByteCountsPatchOffset are rebased to hold ABSOLUTE file offsets, so TiffImageSink works
/// on a multi-IFD file exactly as it does on a single-IFD file (it seeks absolutely for each
/// tile). Requires at least one model. All models must agree on `isBigTiff` (a file's header can
/// only declare one container kind); Error::InvalidArgument if they disagree, if `models` is
/// empty, or if any per-image plan fails.
[[nodiscard]] Result<TiffFileWritePlan> planTiffWriteMulti(std::span<const StorageModel> models);

} // namespace ptiff::io::backend::tiff
