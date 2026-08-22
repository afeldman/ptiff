#include <span>
#include <vector>

#include <ptiff/io/backend/pds4/pds4_document.hpp>

namespace ptiff::io::backend::pds4 {

Result<Pds4Document> readDocument(io::BinaryReader& reader) {
    // A PDS4 document is self-describing from byte 0, and both deserializeModel and
    // openImageSource read it (possibly after a prior read advanced the reader), so always start
    // from the document origin.
    auto reset = reader.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }

    // 1. Seek header: little-endian uint64 giving the label byte length.
    std::vector<std::byte> headerBytes(kHeaderSize);
    auto readHeader = reader.read(std::span<std::byte>{headerBytes});
    if (!readHeader.has_value()) {
        return std::unexpected(readHeader.error());
    }
    if (*readHeader != headerBytes.size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "pds4: truncated seek header"});
    }
    std::uint64_t labelBytes = 0;
    for (std::size_t i = 0; i < kHeaderSize; ++i) {
        labelBytes |= static_cast<std::uint64_t>(std::to_integer<unsigned char>(headerBytes[i]))
                      << (8 * i);
    }
    if (labelBytes == 0 || labelBytes > (1ull << 24)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "pds4: implausible label byte length"});
    }

    // 2. Label text of that length.
    std::vector<std::byte> label(labelBytes);
    auto readLabel = reader.read(std::span<std::byte>{label});
    if (!readLabel.has_value()) {
        return std::unexpected(readLabel.error());
    }
    if (*readLabel != label.size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "pds4: truncated XML label"});
    }

    auto parsed = parseLabel(label);
    if (!parsed.has_value()) {
        return std::unexpected(parsed.error());
    }
    return Pds4Document{std::move(*parsed), labelBytes};
}

} // namespace ptiff::io::backend::pds4
