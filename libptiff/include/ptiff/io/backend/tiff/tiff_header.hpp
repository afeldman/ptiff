#pragma once

#include <cstdint>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/tiff_endian.hpp>
#include <ptiff/io/binary_reader.hpp>

namespace ptiff::io::backend::tiff {

/// Parsed TIFF/BigTIFF file header: byte order, container kind, and the absolute offset of the
/// first IFD.
struct TiffHeader {
    Endian endian = Endian::Little;
    bool isBigTiff = false;
    std::uint64_t firstIfdOffset = 0;
};

/// Reads and validates the 8-byte classic TIFF header or 16-byte BigTIFF header, starting at
/// `reader`'s current position. Error::InvalidArgument if the byte-order mark or magic number is
/// not recognized, the input is shorter than a header, or (BigTIFF only) the offset-byte-size
/// field is not 8.
[[nodiscard]] Result<TiffHeader> readTiffHeader(BinaryReader& reader);

} // namespace ptiff::io::backend::tiff
