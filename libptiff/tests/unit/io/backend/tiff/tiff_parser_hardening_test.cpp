// Deterministic malformed-input hardening tests for the TIFF/BigTIFF read path (RFC-0001 §13).
//
// Each test builds a well-formed classic TIFF in memory, mutates specific bytes to introduce a
// single malformation, and asserts the parser rejects it with ErrorCode::InvalidArgument -- never
// crashing, hanging, or allocating exorbitantly. This is the architectural contract from
// RFC-0001 §13: treat all on-disk offsets/lengths/counts as untrusted and validate before use.

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/core/error.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_header.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/backend/tiff/tiff_image_source.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::Error;
using ptiff::ErrorCode;
using ptiff::Result;
using ptiff::io::backend::TiffBackend;
namespace tiff = ptiff::io::backend::tiff;

class BufferBinaryReader final : public ptiff::io::BinaryReader {
public:
    explicit BufferBinaryReader(std::vector<std::byte> data) : data_(std::move(data)) {}

    ptiff::Result<std::size_t> read(std::span<std::byte> destination) override {
        const std::size_t available = data_.size() - position_;
        const std::size_t toCopy = std::min(destination.size(), available);
        std::copy_n(
            data_.begin() + static_cast<std::ptrdiff_t>(position_), toCopy, destination.begin());
        position_ += toCopy;
        return toCopy;
    }
    ptiff::Result<void> seek(std::uint64_t offset) override {
        if (offset > data_.size()) {
            return std::unexpected(
                ptiff::Error{ptiff::ErrorCode::InvalidArgument, "seek out of range"});
        }
        position_ = offset;
        return {};
    }
    ptiff::Result<std::uint64_t> position() const override { return position_; }
    ptiff::Result<std::uint64_t> size() const override { return data_.size(); }

private:
    std::vector<std::byte> data_;
    std::uint64_t position_ = 0;
};

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

void appendU16LE(std::vector<std::byte>& out, std::uint16_t v) {
    out.push_back(std::byte{static_cast<unsigned char>(v & 0xFF)});
    out.push_back(std::byte{static_cast<unsigned char>((v >> 8) & 0xFF)});
}

void appendU32LE(std::vector<std::byte>& out, std::uint32_t v) {
    out.push_back(std::byte{static_cast<unsigned char>(v & 0xFF)});
    out.push_back(std::byte{static_cast<unsigned char>((v >> 8) & 0xFF)});
    out.push_back(std::byte{static_cast<unsigned char>((v >> 16) & 0xFF)});
    out.push_back(std::byte{static_cast<unsigned char>((v >> 24) & 0xFF)});
}

/// Builds a well-formed classic little-endian, grayscale 2x2, single-strip TIFF.
/// Layout: header (8 bytes) | entry count (2) | 8 entries (12 each) | next-IFD (4) | pixels (4).
/// Total 114 bytes. Entry tags: ImageWidth, ImageLength, BitsPerSample, Compression,
/// PhotometricInterpretation, StripOffsets, RowsPerStrip, StripByteCounts.
std::vector<std::byte> buildValidClassicTiff() {
    std::vector<std::byte> file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});

    appendU16LE(file, 8); // entry count
    // tagId(2), fieldType(2), count(4), value-area(4)
    auto entry =
        [&](std::uint16_t tag, std::uint16_t type, std::uint32_t count, std::uint32_t value) {
            appendU16LE(file, tag);
            appendU16LE(file, type);
            appendU32LE(file, count);
            appendU32LE(file, value);
        };
    entry(256, 3, 1, 2);   // ImageWidth = 2 (SHORT)
    entry(257, 3, 1, 2);   // ImageLength = 2 (SHORT)
    entry(258, 3, 1, 8);   // BitsPerSample = 8 (SHORT)
    entry(259, 3, 1, 1);   // Compression = 1 (None)
    entry(262, 3, 1, 1);   // PhotometricInterpretation = 1 (BlackIsZero)
    entry(273, 4, 1, 110); // StripOffsets = 110 (LONG)
    entry(278, 3, 1, 2);   // RowsPerStrip = 2 (SHORT)
    entry(279, 4, 1, 4);   // StripByteCounts = 4 (LONG)

    appendU32LE(file, 0); // next-IFD offset = 0 (last)
    const auto pixels = bytes({10, 20, 30, 40});
    file.insert(file.end(), pixels.begin(), pixels.end());
    return file;
}

/// Where each field lives (byte offset into the buffer) for the classic baseline layout above.
constexpr std::size_t kEntryCountOffset = 8;
constexpr std::size_t kFirstEntryOffset = 10;
constexpr std::size_t kNextIfdOffset = 106; // entryCount + 8 entries = 10 + 96
constexpr std::size_t kEntrySize = 12;
constexpr std::size_t kMinimalFileSize = 114;

/// Overwrites `count` bytes at `offset` in `file` with `v` (little-endian).
void overwriteU16(std::vector<std::byte>& file, std::size_t offset, std::uint16_t v) {
    REQUIRE(offset + 2 <= file.size());
    file[offset] = std::byte{static_cast<unsigned char>(v & 0xFF)};
    file[offset + 1] = std::byte{static_cast<unsigned char>((v >> 8) & 0xFF)};
}

void overwriteU32(std::vector<std::byte>& file, std::size_t offset, std::uint32_t v) {
    REQUIRE(offset + 4 <= file.size());
    file[offset] = std::byte{static_cast<unsigned char>(v & 0xFF)};
    file[offset + 1] = std::byte{static_cast<unsigned char>((v >> 8) & 0xFF)};
    file[offset + 2] = std::byte{static_cast<unsigned char>((v >> 16) & 0xFF)};
    file[offset + 3] = std::byte{static_cast<unsigned char>((v >> 24) & 0xFF)};
}

/// Runs the full backend read path (header -> IFD chain -> directory interpretation) over `file`
/// and returns the result. Encapsulates the exact entry point deserializeModel uses for the read
/// dir + a fresh reader each time (the reader is cursor-based).
Result<ptiff::io::StorageModel> readViaBackend(std::vector<std::byte> file) {
    BufferBinaryReader reader(std::move(file));
    TiffBackend backend;
    return backend.deserializeModel(reader);
}

/// Asserts that reading `file` fails with ErrorCode::InvalidArgument (rejects, no crash).
void requireRejected(std::vector<std::byte> file) {
    auto result = readViaBackend(std::move(file));
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ErrorCode::InvalidArgument);
}

} // namespace

// ---------------------------------------------------------------------------
// Sanity: the well-formed baseline must itself parse cleanly.
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] a well-formed classic TIFF parses and yields one child",
          "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    REQUIRE(file.size() == kMinimalFileSize);
    auto model = readViaBackend(std::move(file));
    REQUIRE(model.has_value());
    REQUIRE(model->children().size() == 1);
}

// ---------------------------------------------------------------------------
// Header truncation / structural malformation
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] a header shorter than 8 bytes is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    file.resize(4);
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] an unrecognized byte-order mark is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    file[0] = std::byte{'X'};
    file[1] = std::byte{'X'};
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] an unrecognized magic number is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    file[2] = std::byte{0x13};
    file[3] = std::byte{0x00};
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] a first-IFD offset that points outside the file is rejected",
          "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // Header firstIfdOffset is bytes 4-7 (value 8). Set it far beyond the file.
    file[4] = std::byte{0xFF};
    file[5] = std::byte{0xFF};
    file[6] = std::byte{0xFF};
    file[7] = std::byte{0x7F};
    requireRejected(std::move(file));
}

// ---------------------------------------------------------------------------
// Chain hardening: cyclic, self-referential, non-advancing, and flood next-IFD offsets
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] a self-referential (next == same IFD) chain is rejected",
          "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // next-IFD points back at the current IFD (offset 8) instead of 0.
    overwriteU32(file, kNextIfdOffset, 8);
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] a backward (non-advancing) next-IFD is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // next-IFD points below the current IFD offset (into the header).
    overwriteU32(file, kNextIfdOffset, 4);
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] a cyclic two-IFD chain is rejected", "[io][tiff][hardening]") {
    // Build a two-IFD chain where IFD1's next-IFD points back to IFD0 (offset 8).
    auto file = buildValidClassicTiff();
    // The second IFD is appended right after pixel data at offset `file.size()` (now 114).
    // First IFD's next-IFD -> 114. Appended second IFD: entry count 0 + its own next-IFD -> 8
    // (back to IFD0), forming a cycle readDirectoryChain must reject.
    const std::uint32_t secondIfdOffset = static_cast<std::uint32_t>(file.size());
    auto second = bytes({0x00, 0x00}); // entry count = 0
    appendU32LE(second, 8);            // second IFD's next-IFD -> back to 8 (cycle)
    file.insert(file.end(), second.begin(), second.end());
    overwriteU32(file, kNextIfdOffset, secondIfdOffset); // first IFD -> second IFD
    requireRejected(std::move(file));
}

// ---------------------------------------------------------------------------
// IFD entry-count hardening: inflated count, overflow, truncated table
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] an inflated entry count that exceeds the file is rejected",
          "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // Claim 65535 entries when the file can only hold a couple.
    overwriteU16(file, kEntryCountOffset, 0xFFFF);
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] an entry table truncated mid-entry is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // Keep the count but truncate the entry table so the last entry is incomplete.
    file.resize(kNextIfdOffset - 2);
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] a truncated next-IFD field is rejected", "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    // Cut into the next-IFD field (should be 4 bytes).
    file.resize(kNextIfdOffset + 2);
    requireRejected(std::move(file));
}

// ---------------------------------------------------------------------------
// Tag value hardening: out-of-line values overrunning the file, huge counts
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] an out-of-line tag value that overruns the file is rejected",
          "[io][tiff][hardening]") {
    // Replace a LONG single-value tag (StripOffsets, entry index 5) whose count is 1 (inline)
    // with a count=2 LONG, forcing an out-of-line 8-byte value at an offset near EOF.
    auto file = buildValidClassicTiff();
    const std::size_t entryOffset = kFirstEntryOffset + 5 * kEntrySize;
    overwriteU32(file, entryOffset + 4, 2); // count = 2 (for a LONG -> 8 bytes out-of-line)
    // value-area holds the offset to the out-of-line data; point at a place where only 2 bytes
    // remain in the file.
    overwriteU32(file, entryOffset + 8, static_cast<std::uint32_t>(file.size() - 2));
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] a tag claiming an oversized element count is rejected",
          "[io][tiff][hardening]") {
    // BitsPerSample is the 3rd entry (index 2). Set its count to a huge value that overruns the
    // value area; for a valid parse the count must be bounded. Use count = 0xFFFFFFFF as a SHORT
    // array -> the reader must reject it rather than read gigabytes.
    auto file = buildValidClassicTiff();
    const std::size_t entryOffset = kFirstEntryOffset + 2 * kEntrySize;
    overwriteU32(file, entryOffset + 4, 0xFFFFu); // count = 65535
    requireRejected(std::move(file));
}

TEST_CASE("[hardening] an out-of-line tag value offset outside the file is rejected",
          "[io][tiff][hardening]") {
    auto file = buildValidClassicTiff();
    const std::size_t entryOffset = kFirstEntryOffset + 5 * kEntrySize; // StripOffsets
    overwriteU32(file, entryOffset + 4, 2);                             // count = 2 (8-byte value)
    overwriteU32(file, entryOffset + 8, 0xFFFFFF00u);                   // offset beyond EOF
    requireRejected(std::move(file));
}

// ---------------------------------------------------------------------------
// Strip/tile table structural hardening
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] mismatched StripOffsets/StripByteCounts lengths are rejected",
          "[io][tiff][hardening]") {
    // StripByteCounts is the 8th entry (index 7). Bump its count to 2 (a 2-element LONG array)
    // while StripOffsets still has count 1, so interpretTiffIfd sees a length mismatch.
    auto file = buildValidClassicTiff();
    const std::size_t entryOffset = kFirstEntryOffset + 7 * kEntrySize;
    overwriteU32(file, entryOffset + 4, 2); // StripByteCounts count = 2
    // The 8-byte out-of-line value must point somewhere in-bounds; reuse an offset near EOF with
    // 2 LONGs. Append 8 zero bytes to be safe.
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    overwriteU32(file, entryOffset + 8, static_cast<std::uint32_t>(file.size()));
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    file.push_back(std::byte{0});
    requireRejected(std::move(file));
}

// ---------------------------------------------------------------------------
// Pixel/tile bounds hardening (driven through the ImageSource read path)
// ---------------------------------------------------------------------------
TEST_CASE("[hardening] a StripOffsets value pointing past EOF is rejected at the data level",
          "[io][tiff][hardening]") {
    // The directory parses fine, but reading the tile must reject an out-of-bounds byte range.
    auto file = buildValidClassicTiff();
    // StripOffsets entry (index 5) value-area is the strip offset (110). Set it to a value
    // beyond the file so the ImageSource's tile read fails cleanly.
    const std::size_t entryOffset = kFirstEntryOffset + 5 * kEntrySize;
    overwriteU32(file, entryOffset + 8, 0x7FFFFFFFu);

    // deserializeModel still succeeds (offsets are validated lazily at readTile time); assert that
    // reading the tile through openImageSource rejects with InvalidArgument.
    BufferBinaryReader reader(file);
    TiffBackend backend;
    auto source = backend.openImageSource(reader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ErrorCode::InvalidArgument);
}

TEST_CASE("[hardening] a truncated pixel strip is rejected at the data level",
          "[io][tiff][hardening]") {
    // Keep the header/IFD valid but shrink the pixel payload so the strip byte range extends
    // past EOF: set StripOffsets to point at the very end of the file and StripByteCounts=4.
    auto file = buildValidClassicTiff();
    const std::size_t stripOffsetsEntry = kFirstEntryOffset + 5 * kEntrySize;
    overwriteU32(file, stripOffsetsEntry + 8, static_cast<std::uint32_t>(file.size() - 2));
    const std::size_t stripByteCountsEntry = kFirstEntryOffset + 7 * kEntrySize;
    overwriteU32(file, stripByteCountsEntry + 8, 4); // claims 4 bytes but only 2 remain

    BufferBinaryReader reader(file);
    TiffBackend backend;
    auto source = backend.openImageSource(reader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ErrorCode::InvalidArgument);
}
