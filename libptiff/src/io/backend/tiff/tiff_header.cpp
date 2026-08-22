#include <array>
#include <cstddef>

#include <ptiff/core/error.hpp>
#include <ptiff/io/backend/tiff/tiff_header.hpp>

namespace ptiff::io::backend::tiff {

namespace {

constexpr std::uint16_t kClassicMagic = 42;
constexpr std::uint16_t kBigTiffMagic = 43;

Result<Endian> readByteOrder(std::span<const std::byte> mark) {
    const auto first = std::to_integer<unsigned char>(mark[0]);
    const auto second = std::to_integer<unsigned char>(mark[1]);
    if (first == 'I' && second == 'I') {
        return Endian::Little;
    }
    if (first == 'M' && second == 'M') {
        return Endian::Big;
    }
    return std::unexpected(ptiff::Error{ptiff::ErrorCode::InvalidArgument,
                                        "readTiffHeader: unrecognized byte-order mark"});
}

} // namespace

Result<TiffHeader> readTiffHeader(BinaryReader& reader) {
    std::array<std::byte, 16> raw{};
    auto firstRead = reader.read(std::span<std::byte>{raw}.first(8));
    if (!firstRead.has_value()) {
        return std::unexpected(firstRead.error());
    }
    if (*firstRead != 8) {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::InvalidArgument,
                                            "readTiffHeader: file shorter than a TIFF header"});
    }

    auto endian = readByteOrder(std::span<const std::byte>{raw}.first(2));
    if (!endian.has_value()) {
        return std::unexpected(endian.error());
    }

    const auto magic = readU16(std::span<const std::byte>{raw}.subspan(2, 2), *endian);
    if (magic == kClassicMagic) {
        const auto offset = readU32(std::span<const std::byte>{raw}.subspan(4, 4), *endian);
        return TiffHeader{.endian = *endian, .isBigTiff = false, .firstIfdOffset = offset};
    }
    if (magic != kBigTiffMagic) {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::InvalidArgument,
                                            "readTiffHeader: unrecognized magic number"});
    }

    const auto offsetByteSize = readU16(std::span<const std::byte>{raw}.subspan(4, 2), *endian);
    const auto constantZero = readU16(std::span<const std::byte>{raw}.subspan(6, 2), *endian);
    if (offsetByteSize != 8 || constantZero != 0) {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::InvalidArgument,
                                            "readTiffHeader: malformed BigTIFF header fields"});
    }

    auto secondRead = reader.read(std::span<std::byte>{raw}.subspan(8, 8));
    if (!secondRead.has_value()) {
        return std::unexpected(secondRead.error());
    }
    if (*secondRead != 8) {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::InvalidArgument,
                                            "readTiffHeader: file shorter than a BigTIFF header"});
    }

    const auto offset = readU64(std::span<const std::byte>{raw}.subspan(8, 8), *endian);
    return TiffHeader{.endian = *endian, .isBigTiff = true, .firstIfdOffset = offset};
}

} // namespace ptiff::io::backend::tiff
