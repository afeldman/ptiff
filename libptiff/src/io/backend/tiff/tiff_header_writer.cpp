#include <array>
#include <cstddef>
#include <span>

#include <ptiff/io/backend/tiff/tiff_endian.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>

namespace ptiff::io::backend::tiff {

namespace {

Result<void> writeAndCheck(BinaryWriter& writer, std::span<const std::byte> header) {
    auto result = writer.write(header);
    if (!result.has_value()) {
        return std::unexpected(result.error());
    }
    if (*result != header.size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "writeTiffHeader: short write"});
    }
    return {};
}

} // namespace

Result<void> writeTiffHeader(BinaryWriter& writer, std::uint64_t firstIfdOffset, bool isBigTiff) {
    if (!isBigTiff) {
        std::array<std::byte, kClassicTiffHeaderSize> header{
            std::byte{'I'}, std::byte{'I'}, std::byte{0x2A}, std::byte{0x00}};
        writeU32(std::span<std::byte>{header}.subspan(4, 4),
                 static_cast<std::uint32_t>(firstIfdOffset),
                 Endian::Little);
        return writeAndCheck(writer, header);
    }

    std::array<std::byte, kBigTiffHeaderSize> header{
        std::byte{'I'}, std::byte{'I'}, std::byte{0x2B}, std::byte{0x00}};
    writeU16(std::span<std::byte>{header}.subspan(4, 2), 8, Endian::Little); // offsetByteSize
    writeU16(std::span<std::byte>{header}.subspan(6, 2), 0, Endian::Little); // constant
    writeU64(std::span<std::byte>{header}.subspan(8, 8), firstIfdOffset, Endian::Little);
    return writeAndCheck(writer, header);
}

} // namespace ptiff::io::backend::tiff
