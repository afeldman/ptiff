#include <span>
#include <string_view>
#include <vector>

#include <ptiff/io/backend/isis/isis_document.hpp>

namespace ptiff::io::backend::isis {

namespace {
// Upper bound of how much of the front we scan for the label's "End" marker. The pixel block is
// addressed right after it, so we never need to read the whole file for the label.
constexpr std::uint64_t kMaxLabelScan = 4u * 1024u * 1024u; // 4 MiB
} // namespace

Result<IsisDocument> readDocument(io::BinaryReader& reader) {
    auto sizeResult = reader.size();
    if (!sizeResult.has_value()) {
        return std::unexpected(sizeResult.error());
    }
    const std::uint64_t scan = (*sizeResult < kMaxLabelScan) ? *sizeResult : kMaxLabelScan;

    auto reset = reader.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }
    std::vector<std::byte> buffer(static_cast<std::size_t>(scan));
    auto readResult = reader.read(std::span<std::byte>{buffer});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    const std::size_t got = *readResult;
    const std::string_view text(reinterpret_cast<const char*>(buffer.data()), got);

    // The label ends with a bare "End\n" line. Because the only such line is the final one
    // (End_Object has a trailing underscore), the last "\nEnd\n" marks the pixel origin.
    const std::string_view endMarker = "\nEnd\n";
    const std::size_t markerPos = text.rfind(endMarker);
    if (markerPos == std::string_view::npos) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "isis: no End marker closing the label"});
    }
    const std::uint64_t labelBytes = static_cast<std::uint64_t>(markerPos + endMarker.size());

    auto parsed = parseLabel(std::span<const std::byte>{buffer}.first(labelBytes));
    if (!parsed.has_value()) {
        return std::unexpected(parsed.error());
    }
    return IsisDocument{std::move(*parsed), labelBytes};
}

} // namespace ptiff::io::backend::isis
