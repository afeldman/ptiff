// Baseline / container conformance tests for the PTIFF reference implementation.
//
// Part of the PTIFF conformance suite (see conformance/ in the repo root and
// conformance/levels.md Level 0). These tests validate the bottom rung of the
// conformance ladder: the file MUST be parseable as a valid classic TIFF or
// BigTIFF file, expose a sound IFD structure, and let a generic reader locate
// and decode the baseline image using only standard TIFF/BigTIFF mechanisms.
//
// The tests deliberately build raw TIFF/BigTIFF bytes by hand rather than using
// the library's own writer, so they exercise the reader against the *spec* --
// not against the implementation's own round-trip assumptions.
//
// Tags mirroring the requirement IDs in conformance/levels.md:
//   [conformance][baseline]  -- whole Level 0 block, run via ctest -R conformance
//                               plus this tag for targeted runs.

#include <array>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <span>
#include <string>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

// ---------------------------------------------------------------------------
// Byte-writing helpers (little-endian layouts used by TIFF/BigTIFF).
// ---------------------------------------------------------------------------

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

void appendU16(std::vector<std::byte>& out, std::uint16_t v) {
    out.push_back(static_cast<std::byte>(v & 0xFF));
    out.push_back(static_cast<std::byte>((v >> 8) & 0xFF));
}

void appendU32(std::vector<std::byte>& out, std::uint32_t v) {
    out.push_back(static_cast<std::byte>(v & 0xFF));
    out.push_back(static_cast<std::byte>((v >> 8) & 0xFF));
    out.push_back(static_cast<std::byte>((v >> 16) & 0xFF));
    out.push_back(static_cast<std::byte>((v >> 24) & 0xFF));
}

void appendU64(std::vector<std::byte>& out, std::uint64_t v) {
    for (int i = 0; i < 8; ++i) {
        out.push_back(static_cast<std::byte>((v >> (8 * i)) & 0xFF));
    }
}

constexpr std::uint16_t kTagImageWidth = 256;
constexpr std::uint16_t kTagImageLength = 257;
constexpr std::uint16_t kTagBitsPerSample = 258;
constexpr std::uint16_t kTagCompression = 259;
constexpr std::uint16_t kTagPhotometric = 262;
constexpr std::uint16_t kTagStripOffsets = 273;
constexpr std::uint16_t kTagRowsPerStrip = 278;
constexpr std::uint16_t kTagStripByteCounts = 279;

// ---------------------------------------------------------------------------
// Little-endian classic TIFF writer (hand-rolled, deterministic).
// ---------------------------------------------------------------------------

// Builds a classic little-endian TIFF buffer from a list of IFD entries. Each
// entry is {tag, type, count, value} where `value` is the raw inline value/offset.
// Requires exactly one image (entries) and a following pixel blob placed at the
// byte offset given by `pixelDataOffset`. Returns the complete file buffer.
std::vector<std::byte> makeClassicTiff(std::span<const std::array<std::uint32_t, 4>> entries,
                                       std::span<const std::byte> pixels,
                                       std::uint32_t firstIfdOffset) {
    std::vector<std::byte> file;
    // Header: little-endian 'II', magic 42, offset to first IFD.
    file.push_back(static_cast<std::byte>('I'));
    file.push_back(static_cast<std::byte>('I'));
    appendU16(file, 42);
    appendU32(file, firstIfdOffset);

    // Pad header area up to the IFD offset.
    REQUIRE(file.size() <= firstIfdOffset);
    while (file.size() < firstIfdOffset) {
        file.push_back(std::byte{0});
    }

    // IFD: entry count, then entries, then next-IFD offset (0 => no more).
    appendU16(file, static_cast<std::uint16_t>(entries.size()));
    for (const auto& e : entries) {
        appendU16(file, static_cast<std::uint16_t>(e[0]));
        appendU16(file, static_cast<std::uint16_t>(e[1]));
        appendU32(file, e[2]);
        appendU32(file, e[3]);
    }
    appendU32(file, 0); // no next IFD

    // Align pixel data to a word boundary for realism.
    while (file.size() % 2 != 0) {
        file.push_back(std::byte{0});
    }
    file.insert(file.end(), pixels.begin(), pixels.end());
    return file;
}

std::filesystem::path writeTempFile(const std::string& name, std::vector<std::byte>&& data) {
    auto path = std::filesystem::temp_directory_path() / name;
    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    stream.write(reinterpret_cast<const char*>(data.data()),
                 static_cast<std::streamsize>(data.size()));
    stream.close();
    return path;
}

} // namespace

// ===========================================================================
// L0.2 -- byte order + magic
// ===========================================================================

TEST_CASE("conformance: recognizes little-endian classic TIFF magic (42)",
          "[conformance][baseline]") {
    // A minimal but structurally valid little-endian classic TIFF with a
    // correct IFD and an in-bounds strip offset.
    auto file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto entries = std::to_array<std::array<std::uint32_t, 4>>({
        {kTagImageWidth, 3, 1, 2},
        {kTagImageLength, 3, 1, 2},
        {kTagBitsPerSample, 3, 1, 8},
        {kTagCompression, 3, 1, 1},
        {kTagPhotometric, 3, 1, 1},
        {kTagStripOffsets, 4, 1, 0}, // patched to the real pixel offset
        {kTagRowsPerStrip, 3, 1, 2},
        {kTagStripByteCounts, 4, 1, 4},
    });
    std::array<std::byte, 4> pixels{std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}};
    // 8 hdr + 2 count + 8*12 entries + 4 next = 110 -> pixels at 110.
    auto built = makeClassicTiff(entries, pixels, 8);
    std::uint32_t pixelOffset = 110;
    // StripOffsets entry is index 5; entry starts at 8+2+5*12 = 70; value bytes 8..11.
    built[70 + 8] = static_cast<std::byte>(pixelOffset & 0xFF);
    built[70 + 9] = static_cast<std::byte>((pixelOffset >> 8) & 0xFF);
    built[70 + 10] = static_cast<std::byte>((pixelOffset >> 16) & 0xFF);
    built[70 + 11] = static_cast<std::byte>((pixelOffset >> 24) & 0xFF);

    auto path = writeTempFile("conformance_classic_le.tif", std::move(built));
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**readerResult);
    REQUIRE(model.has_value());
    auto& child = model->children().front();
    REQUIRE(child.field("imageWidth").value() == "2");
    REQUIRE(child.field("imageHeight").value() == "2");

    // The pixel payload is also reachable (L0.5 sanity on the same file).
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto source = backend.openImageSource(**pixelReaderResult);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

TEST_CASE("conformance: rejects a wrong header signature", "[conformance][baseline]") {
    // Header claims big-endian 'MM' but the magic is not followed by a valid
    // big-endian directory; more importantly the classic magic must be 42.
    auto file = bytes({'I', 'I', 0x2B, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto path = writeTempFile("conformance_bad_magic.tif", std::move(file));
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**readerResult);
    // Wrong magic (0x2B instead of 0x2A) must not deserialize successfully.
    REQUIRE_FALSE(model.has_value());
    std::filesystem::remove(path);
}

// ===========================================================================
// L0.1 / L0.3 / L0.4 -- structural validity and baseline tag consistency
// ===========================================================================

TEST_CASE("conformance: full classic TIFF with a valid IFD opens and reports "
          "baseline tags",
          "[conformance][baseline]") {
    auto file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto entries = std::to_array<std::array<std::uint32_t, 4>>({
        {kTagImageWidth, 3, 1, 3},
        {kTagImageLength, 3, 1, 2},
        {kTagBitsPerSample, 3, 1, 8},
        {kTagCompression, 3, 1, 1},
        {kTagPhotometric, 3, 1, 1},
        {kTagStripOffsets, 4, 1, 0}, // patched below
        {kTagRowsPerStrip, 3, 1, 2},
        {kTagStripByteCounts, 4, 1, 6},
    });
    std::array<std::byte, 6> pixels{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    constexpr std::uint32_t kIfd = 8;
    auto built = makeClassicTiff(entries, pixels, kIfd);
    // 8 + 2 + 8*12 + 4 = 8 + 2 + 96 + 4 = 110 (even) -> pixels at 110.
    // Patch the strip offset entry (index 5) in-place to the pixel offset 110.
    // Entry starts at 8(hdr)+2(count)+5*12 = 8+2+60 = 70.
    REQUIRE(built.size() >= 110 + 6);
    std::uint32_t pixelOffset = 110;
    built[70 + 8] = static_cast<std::byte>(pixelOffset & 0xFF);
    built[70 + 9] = static_cast<std::byte>((pixelOffset >> 8) & 0xFF);
    built[70 + 10] = static_cast<std::byte>((pixelOffset >> 16) & 0xFF);
    built[70 + 11] = static_cast<std::byte>((pixelOffset >> 24) & 0xFF);

    auto path = writeTempFile("conformance_classic_full.tif", std::move(built));
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**readerResult);
    REQUIRE(model.has_value());
    REQUIRE(model->children().size() == 1);
    auto& child = model->children().front();
    REQUIRE(child.field("imageWidth").value() == "3");
    REQUIRE(child.field("imageHeight").value() == "2");
    REQUIRE(child.field("samplesPerPixel").value() == "1");
    REQUIRE(child.field("pixelType").value() == "UInt8");
    std::filesystem::remove(path);
}

// ===========================================================================
// L0.5 -- pixel payload reads back byte-for-byte (classic TIFF)
// ===========================================================================

TEST_CASE("conformance: grayscale pixel bytes read back exactly", "[conformance][baseline]") {
    auto file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto entries = std::to_array<std::array<std::uint32_t, 4>>({
        {kTagImageWidth, 3, 1, 2},
        {kTagImageLength, 3, 1, 2},
        {kTagBitsPerSample, 3, 1, 8},
        {kTagCompression, 3, 1, 1},
        {kTagPhotometric, 3, 1, 1},
        {kTagStripOffsets, 4, 1, 0},
        {kTagRowsPerStrip, 3, 1, 2},
        {kTagStripByteCounts, 4, 1, 4},
    });
    std::array<std::byte, 4> pixels{
        std::byte{0xDE}, std::byte{0xAD}, std::byte{0xBE}, std::byte{0xEF}};
    constexpr std::uint32_t kIfd = 8;
    auto built = makeClassicTiff(entries, pixels, kIfd);
    // 8 + 2 + 8*12 + 4 = 110 -> pixels at 110.
    std::uint32_t pixelOffset = 110;
    built[70 + 8] = static_cast<std::byte>(pixelOffset & 0xFF);
    built[70 + 9] = static_cast<std::byte>((pixelOffset >> 8) & 0xFF);
    built[70 + 10] = static_cast<std::byte>((pixelOffset >> 16) & 0xFF);
    built[70 + 11] = static_cast<std::byte>((pixelOffset >> 24) & 0xFF);

    auto path = writeTempFile("conformance_gray_pixels.tif", std::move(built));

    // Deserialize.
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**readerResult);
    REQUIRE(model.has_value());

    // Read pixels on a fresh cursor.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto source = backend.openImageSource(**pixelReaderResult);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

// ===========================================================================
// L0.1 / L0.2 / L0.5 -- BigTIFF (magic 43, 8-byte offsets)
// ===========================================================================

TEST_CASE("conformance: recognizes BigTIFF magic (43) and 8-byte offsets",
          "[conformance][baseline]") {
    // Hand-rolled minimal BigTIFF: header is 16 bytes.
    //   'II' 43 00, offset-size = 8, reserved = 0 0, 0x10,0x00,0x00,0x00,0x00,0x00,0x00,0x00
    // Then BigTIFF IFD: u64 entryCount, each entry 20 bytes, u64 nextIFD.
    std::vector<std::byte> file;
    file.push_back(static_cast<std::byte>('I'));
    file.push_back(static_cast<std::byte>('I'));
    appendU16(file, 43); // BigTIFF magic
    appendU16(file, 8);  // offset size (8 => standard BigTIFF)
    appendU16(file, 0);  // reserved (must be 0)
    appendU64(file, 16); // first IFD offset (64-bit), right after the 16-byte header
    REQUIRE(file.size() == 16);

    std::uint64_t entryCount = 8;
    appendU64(file, entryCount);

    // BigTIFF entry: tag(u16), type(u16), count(u64), value/offset(u64) = 20 bytes.
    auto appendBigEntry =
        [&file](std::uint16_t tag, std::uint16_t type, std::uint64_t count, std::uint64_t value) {
            appendU16(file, tag);
            appendU16(file, type);
            appendU64(file, count);
            appendU64(file, value);
        };
    // The full baseline tag set required by interpretTiffIfd (strip-based layout
    // needs RowsPerStrip).
    appendBigEntry(kTagImageWidth, 3, 1, 2);
    appendBigEntry(kTagImageLength, 3, 1, 2);
    appendBigEntry(kTagBitsPerSample, 3, 1, 8);
    appendBigEntry(kTagCompression, 3, 1, 1);  // Compression = 1 (None)
    appendBigEntry(kTagPhotometric, 3, 1, 1);  // Photometric = 1 (BlackIsZero)
    appendBigEntry(kTagStripOffsets, 4, 1, 0); // patched to pixel offset (index 5)
    appendBigEntry(kTagStripByteCounts, 4, 1, 4);
    appendBigEntry(kTagRowsPerStrip, 3, 1, 2);

    appendU64(file, 0); // no next IFD

    // After header(16) + count(8) + 8*20 + next(8) = 16 + 8 + 160 + 8 = 192.
    // Align to even (already even). Pixels at 192.
    std::uint64_t pixelOffset = file.size();
    REQUIRE(pixelOffset == 192);

    // Patch the StripOffsets entry (index 5). Entry starts at
    // 16 (hdr) + 8 (count) = 24, plus 5*20 = 100 => 124. The BigTIFF entry layout is
    // tag(2) + type(2) + count(8) + value/offset(8), so the u64 value sits at bytes
    // 124+12 .. 124+19 = 136 .. 143.
    constexpr std::uint64_t kStripEntryByte = 124;
    file[kStripEntryByte + 12] = static_cast<std::byte>(pixelOffset & 0xFF);
    file[kStripEntryByte + 13] = static_cast<std::byte>((pixelOffset >> 8) & 0xFF);
    file[kStripEntryByte + 14] = static_cast<std::byte>((pixelOffset >> 16) & 0xFF);
    file[kStripEntryByte + 15] = static_cast<std::byte>((pixelOffset >> 24) & 0xFF);
    file[kStripEntryByte + 16] = static_cast<std::byte>((pixelOffset >> 32) & 0xFF);
    file[kStripEntryByte + 17] = static_cast<std::byte>((pixelOffset >> 40) & 0xFF);
    file[kStripEntryByte + 18] = static_cast<std::byte>((pixelOffset >> 48) & 0xFF);
    file[kStripEntryByte + 19] = static_cast<std::byte>((pixelOffset >> 56) & 0xFF);

    std::array<std::byte, 4> pixels{std::byte{0}, std::byte{1}, std::byte{2}, std::byte{3}};
    file.insert(file.end(), pixels.begin(), pixels.end());

    auto path = writeTempFile("conformance_bigtiff.tif", std::move(file));
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**readerResult);
    REQUIRE(model.has_value());
    REQUIRE(model->children().size() == 1);
    auto& child = model->children().front();
    REQUIRE(child.field("imageWidth").value() == "2");
    REQUIRE(child.field("imageHeight").value() == "2");

    // Read pixels back byte-for-byte on a fresh cursor.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto source = backend.openImageSource(**pixelReaderResult);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}
