// Golden-file conformance test for the reference implementation.
//
// This is the project's first deterministic "golden" anchor: it serializes a
// fixed, versioned Scene to a classic TIFF file (with a deterministic pixel
// provider) and asserts the produced bytes match an agreed, stable SHA-256
// digest ("golden hash"). Because the writer is deterministic, a change to any
// part of serialization (header, IFD layout, tag encoding, pixel packing) that
// alters the on-disk representation breaks this test and forces a deliberate,
// reviewed bump of the golden hash in `kGoldenRoundTripSha256`.
//
// This deliberately guards the *stability* of the format (a conformance/format
// contract), separate from the round-trip *reliability* tests in the unit
// suite.

#include <array>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <span>
#include <string>
#include <vector>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile_provider.hpp>
#include <ptiff/io/writer.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::PixelType;
using ptiff::Reader;
using ptiff::Scene;
using ptiff::Writer;
using ptiff::io::TileProvider;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

// ---------------------------------------------------------------------------
// Minimal, dependency-free SHA-256 (FIPS 180-4). Kept small and self-contained
// so the golden test has no extra binary/dependency surface; correctness is
// cross-checked against a well-known empty-input digest in the test itself.
// ---------------------------------------------------------------------------
constexpr std::uint32_t rotr32(std::uint32_t x, unsigned n) {
    return (x >> n) | (x << (32 - n));
}

constexpr std::array<std::uint32_t, 64> kSha256K = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};

std::array<std::uint8_t, 32> sha256(const std::vector<std::uint8_t>& msg) {
    std::vector<std::uint8_t> data = msg;
    const std::uint64_t bitlen = static_cast<std::uint64_t>(data.size()) * 8;
    data.push_back(0x80);
    while (data.size() % 64 != 56) {
        data.push_back(0x00);
    }
    for (int i = 7; i >= 0; --i) {
        data.push_back(static_cast<std::uint8_t>((bitlen >> (i * 8)) & 0xff));
    }

    std::array<std::uint32_t, 8> h{0x6a09e667,
                                   0xbb67ae85,
                                   0x3c6ef372,
                                   0xa54ff53a,
                                   0x510e527f,
                                   0x9b05688c,
                                   0x1f83d9ab,
                                   0x5be0cd19};

    for (std::size_t off = 0; off < data.size(); off += 64) {
        std::array<std::uint32_t, 64> w{};
        for (std::size_t i = 0; i < 16; ++i) {
            const std::size_t base = off + i * 4;
            w[i] = (static_cast<std::uint32_t>(data[base]) << 24) |
                   (static_cast<std::uint32_t>(data[base + 1]) << 16) |
                   (static_cast<std::uint32_t>(data[base + 2]) << 8) |
                   (static_cast<std::uint32_t>(data[base + 3]));
        }
        for (std::size_t i = 16; i < 64; ++i) {
            const std::uint32_t s0 =
                rotr32(w[i - 15], 7) ^ rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
            const std::uint32_t s1 = rotr32(w[i - 2], 17) ^ rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }

        std::uint32_t a = h[0], b = h[1], c = h[2], d = h[3];
        std::uint32_t e = h[4], f = h[5], g = h[6], hh = h[7];
        for (std::size_t i = 0; i < 64; ++i) {
            const std::uint32_t S1 = rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25);
            const std::uint32_t ch = (e & f) ^ (~e & g);
            const std::uint32_t temp1 = hh + S1 + ch + kSha256K[i] + w[i];
            const std::uint32_t S0 = rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22);
            const std::uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            const std::uint32_t temp2 = S0 + maj;
            hh = g;
            g = f;
            f = e;
            e = d + temp1;
            d = c;
            c = b;
            b = a;
            a = temp1 + temp2;
        }
        h[0] += a;
        h[1] += b;
        h[2] += c;
        h[3] += d;
        h[4] += e;
        h[5] += f;
        h[6] += g;
        h[7] += hh;
    }

    std::array<std::uint8_t, 32> out{};
    for (std::size_t i = 0; i < 8; ++i) {
        out[i * 4 + 0] = static_cast<std::uint8_t>(h[i] >> 24);
        out[i * 4 + 1] = static_cast<std::uint8_t>(h[i] >> 16);
        out[i * 4 + 2] = static_cast<std::uint8_t>(h[i] >> 8);
        out[i * 4 + 3] = static_cast<std::uint8_t>(h[i]);
    }
    return out;
}

std::string hex(const std::array<std::uint8_t, 32>& d) {
    static constexpr char kHex[] = "0123456789abcdef";
    std::string s;
    s.reserve(64);
    for (std::uint8_t b : d) {
        s.push_back(kHex[b >> 4]);
        s.push_back(kHex[b & 0x0f]);
    }
    return s;
}

std::vector<std::uint8_t> readFileBytes(const std::string& path) {
    std::FILE* f = std::fopen(path.c_str(), "rb");
    REQUIRE(f != nullptr);
    std::vector<std::uint8_t> bytes;
    std::uint8_t buf[4096];
    std::size_t n;
    while ((n = std::fread(buf, 1, sizeof(buf), f)) > 0) {
        bytes.insert(bytes.end(), buf, buf + n);
    }
    std::fclose(f);
    return bytes;
}

// Deterministic, owned-buffer TileProvider: the single 16x16 strip (1 grid
// tile) is filled with the byte value `0x00..0xFF` per position so a change in
// pixel packing shows in the digest.
class GoldenStripProvider final : public TileProvider {
public:
    explicit GoldenStripProvider(TileLayout layout) : layout_(layout), pixels_(256, std::byte{0}) {
        for (std::size_t i = 0; i < pixels_.size(); ++i) {
            pixels_[i] = std::byte(static_cast<unsigned char>(i % 256));
        }
    }

    [[nodiscard]] const TileLayout& layout() const noexcept override { return layout_; }

    [[nodiscard]] ptiff::Result<Tile> provideTile(const TileIndex& index) override {
        auto region = layout_.regionFor(index);
        if (!region.has_value()) {
            return std::unexpected(region.error());
        }
        return Tile(ptiff::TileId{0},
                    index,
                    *region,
                    std::span<const std::byte>(pixels_.data(), pixels_.size()));
    }

private:
    TileLayout layout_;
    std::vector<std::byte> pixels_;
};

Scene goldenScene() {
    Scene scene;
    ImageDescriptor d;
    d.width = 16;
    d.height = 16;
    d.pixelType = PixelType::UInt8;
    d.channelCount = 1;
    d.compression = CompressionKind::None;
    (void)scene.addImage(d);
    return scene;
}

// The agreed golden digest for the classic-TIFF serialization of `goldenScene()`
// written via `GoldenStripProvider`. Bump ONLY via a deliberate, reviewed change
// (see the header comment). Value captured 2026-08-11 on the 0.2.0 release commit.
inline constexpr const char* kGoldenRoundTripSha256 =
    "11337e1b58756a9aab4e71f68538f9873cd0f15a893936a74b15cec0ca36c15c";

} // namespace

TEST_CASE("TIFF round-trip serialization matches the golden SHA-256 digest", "[golden]") {
    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "ptiff_golden_roundtrip.tif";

    {
        auto writer = Writer::create(path.string());
        REQUIRE(writer.has_value());
        Scene scene = goldenScene();

        const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                                .imageWidth = 16,
                                .imageHeight = 16,
                                .levelCount = 1};
        GoldenStripProvider provider{layout};
        REQUIRE((*writer)->write(scene, provider).has_value());
    }

    std::vector<std::uint8_t> bytes = readFileBytes(path.string());
    std::string digest = hex(sha256(bytes));
    std::filesystem::remove(path);

    CAPTURE(digest);

    // Test vector sanity: SHA-256("") is a well-known constant. Guards the
    // helper against a trivial bug making every digest identical.
    REQUIRE(hex(sha256({})) == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

    if (std::string(kGoldenRoundTripSha256).empty()) {
        WARN("Golden digest not set yet; current value is: " << digest);
        return;
    }
    REQUIRE(digest == kGoldenRoundTripSha256);
}

/// Golden anchor for the PTIFF extension metadata emission (RFC-7002): a fixed model carrying
/// all five private tags (65001-65005) must serialize to exactly these bytes. This locks the
/// versioned payload codec's layout independently of the scene-level golden above. Follow the
/// same discipline as kGoldenRoundTripSha256: bump ONLY on a deliberate, reviewed format change.
namespace {
inline constexpr const char* kGoldenPtiffTagsSha256 =
    "8591776df8d0a0078f7a2d4133e46aa4bba64b56ccb5f11c9bbaee5d9409e853";
} // namespace

TEST_CASE("PTIFF metadata tags serialize to the golden SHA-256 digest", "[golden][tiff-ptiff]") {
    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "ptiff_golden_tags.tif";

    ptiff::io::StorageModel model;
    model.setField("imageWidth", "16");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    // Deterministic multilingual-free values so the bytes are stable across platforms/locales.
    model.setField("ptiff.spice.frame", "IAU_MOON");
    model.setField("ptiff.spice.time_system", "TDB");
    model.setField("ptiff.camera.model", "pinhole");
    model.setField("ptiff.camera.focal_length_x", "100.0");
    model.setField("ptiff.crs.body", "301");
    model.setField("ptiff.layers.dem", "dem");
    model.setField("ptiff.provenance.software", "libptiff");

    {
        auto writer = ptiff::io::FileBinaryWriter::create(path.string());
        REQUIRE(writer.has_value());
        ptiff::io::backend::TiffBackend backend;
        REQUIRE(backend.serializeModel(model, **writer).has_value());
        REQUIRE((**writer).flush().has_value());
    }

    std::vector<std::uint8_t> bytes = readFileBytes(path.string());
    std::string digest = hex(sha256(bytes));
    std::filesystem::remove(path);

    CAPTURE(digest);

    if (std::string(kGoldenPtiffTagsSha256).empty()) {
        WARN("PTIFF metadata-tags golden digest not set yet; current value is: " << digest);
        return;
    }
    REQUIRE(digest == kGoldenPtiffTagsSha256);
}
