#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <vector>

#include <unistd.h>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile_provider.hpp>
#include <ptiff/io/writer.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::ImageId;
using ptiff::PixelType;
using ptiff::Reader;
using ptiff::Scene;
using ptiff::Writer;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

class TempFile {
public:
    TempFile()
        : path_(std::filesystem::temp_directory_path() /
                ("ptiff_read_pixels_" + std::to_string(::getpid()) + "_" +
                 std::to_string(counter_++) + ".ptiff")) {}
    ~TempFile() { std::remove(path_.c_str()); }
    const std::string& path() const { return path_; }

private:
    static int counter_;
    std::string path_;
};
int TempFile::counter_ = 0;

// A deterministic TileProvider backed by an owned pixel buffer. Each grid tile i is filled with
// the constant byte value `(i + baseValue) % 256`, so a round trip can verify each tile's bytes
// exactly, and two providers can carry distinct payloads via `baseValue`.
class PatternTileProvider final : public ptiff::io::TileProvider {
public:
    PatternTileProvider(TileLayout layout, std::uint32_t tileBytes, std::uint8_t baseValue = 0)
        : layout_(layout),
          tiles_(layout_.columns() * layout_.rows()),
          tileBytes_(tileBytes),
          pixels_(std::size_t(tiles_) * tileBytes_, std::byte{0}) {
        for (std::uint32_t i = 0; i < tiles_; ++i) {
            const std::byte v = std::byte(static_cast<unsigned char>((i + baseValue) % 256));
            for (std::size_t j = std::size_t(i) * tileBytes_; j < std::size_t(i + 1) * tileBytes_;
                 ++j) {
                pixels_[j] = v;
            }
        }
    }

    [[nodiscard]] const TileLayout& layout() const noexcept override { return layout_; }

    [[nodiscard]] ptiff::Result<Tile> provideTile(const TileIndex& index) override {
        auto region = layout_.regionFor(index);
        if (!region.has_value()) {
            return std::unexpected(std::move(region.error()));
        }
        const std::uint32_t columns = layout_.columns(index.level);
        const std::uint32_t linear = index.row * columns + index.column;
        if (linear >= tiles_) {
            return std::unexpected(ptiff::Error{ptiff::ErrorCode::OutOfRange,
                                                "PatternTileProvider: index outside grid"});
        }
        const std::size_t offset = std::size_t(linear) * tileBytes_;
        return Tile(ptiff::TileId{linear},
                    index,
                    *region,
                    std::span<const std::byte>(pixels_.data() + offset, tileBytes_));
    }

private:
    TileLayout layout_;
    std::uint32_t tiles_;
    std::uint32_t tileBytes_;
    std::vector<std::byte> pixels_;
};

Scene buildScene(std::uint16_t width,
                 std::uint16_t height,
                 std::uint32_t channels,
                 bool tiled,
                 std::optional<std::uint32_t> tileSize = std::nullopt,
                 CompressionKind compression = CompressionKind::None) {
    Scene scene;
    ImageDescriptor gs;
    gs.width = width;
    gs.height = height;
    gs.pixelType = PixelType::UInt8;
    gs.channelCount = channels;
    gs.compression = compression;
    if (tiled && tileSize.has_value()) {
        gs.tileInfo = ptiff::TileInfo{.tileWidth = *tileSize, .tileHeight = *tileSize};
    }
    (void)scene.addImage(gs);
    return scene;
}

// Reads a file's pixels through the *Reader facade* (imageSource -> readTile), concatenating
// each tile's payload into a flat vector in grid order (row-major, then level). This exercises
// exactly the public, backend-agnostic read path that this phase adds.
ptiff::Result<std::vector<std::byte>> readPixelsViaReader(const std::string& path,
                                                          ImageId imageId = ImageId{0}) {
    auto reader = Reader::open(path);
    if (!reader.has_value()) {
        return std::unexpected(reader.error());
    }
    auto source = (*reader)->imageSource(imageId);
    if (!source.has_value()) {
        return std::unexpected(source.error());
    }

    const TileLayout& layout = (*source)->layout();
    std::vector<std::byte> out;
    for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
        const std::uint32_t rows = layout.rows(level);
        const std::uint32_t cols = layout.columns(level);
        for (std::uint32_t row = 0; row < rows; ++row) {
            for (std::uint32_t col = 0; col < cols; ++col) {
                auto tile =
                    (*source)->readTile(TileIndex{.column = col, .row = row, .level = level});
                if (!tile.has_value()) {
                    return std::unexpected(tile.error());
                }
                const auto data = tile->data();
                out.insert(out.end(), data.begin(), data.end());
            }
        }
    }
    return out;
}

// Builds the expected flat pixel vector the PatternTileProvider produces for `layout` and
// `tileBytes`: each grid tile i contributes `tileBytes` copies of byte value `(i + baseValue)
// % 256`.
std::vector<std::byte>
expectedPattern(const TileLayout& layout, std::uint32_t tileBytes, std::uint8_t baseValue = 0) {
    std::vector<std::byte> out;
    const std::uint32_t tiles = layout.columns() * layout.rows();
    for (std::uint32_t i = 0; i < tiles; ++i) {
        const std::byte v = std::byte(static_cast<unsigned char>((i + baseValue) % 256));
        out.insert(out.end(), std::size_t(tileBytes), v);
    }
    return out;
}

} // namespace

TEST_CASE("Reader::imageSource round-trips stripped pixel data", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(16, 16, 1, /*tiled=*/false);
    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 16,
                            .imageHeight = 16,
                            .levelCount = 1};
    PatternTileProvider provider{layout, /*tileBytes=*/256};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    auto writeResult = (*writer)->write(scene, provider);
    REQUIRE(writeResult.has_value());

    auto pixels = readPixelsViaReader(tmp.path());
    REQUIRE(pixels.has_value());
    const auto expected = expectedPattern(layout, 256);
    REQUIRE(pixels->size() == expected.size());
    REQUIRE(std::equal(pixels->begin(), pixels->end(), expected.begin()));
}

TEST_CASE("Reader::imageSource round-trips tiled pixel data", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(32, 24, 1, /*tiled=*/true, /*tileSize=*/16);
    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 32,
                            .imageHeight = 24,
                            .levelCount = 1};
    PatternTileProvider provider{layout, /*tileBytes=*/256};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    auto writeResult = (*writer)->write(scene, provider);
    REQUIRE(writeResult.has_value());

    auto pixels = readPixelsViaReader(tmp.path());
    REQUIRE(pixels.has_value());
    const auto expected = expectedPattern(layout, 256); // 2x2 grid of 0x00,0x01,0x02,0x03
    REQUIRE(pixels->size() == expected.size());
    REQUIRE(std::equal(pixels->begin(), pixels->end(), expected.begin()));
}

TEST_CASE("Reader::imageSource round-trips compressed pixel data", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(16, 16, 1, /*tiled=*/false, std::nullopt, CompressionKind::Lzw);
    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 16,
                            .imageHeight = 16,
                            .levelCount = 1};
    PatternTileProvider provider{layout, /*tileBytes=*/256};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    auto writeResult = (*writer)->write(scene, provider);
    REQUIRE(writeResult.has_value());

    // imageSource must decode (LZW here) and surface the raw pixel bytes.
    auto pixels = readPixelsViaReader(tmp.path());
    REQUIRE(pixels.has_value());
    const auto expected = expectedPattern(layout, 256);
    REQUIRE(std::equal(pixels->begin(), pixels->end(), expected.begin()));
}

TEST_CASE("Reader::imageSource round-trips a multi-image scene", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(16, 16, 1, /*tiled=*/false);
    ImageDescriptor second;
    second.width = 16;
    second.height = 16;
    second.pixelType = PixelType::UInt8;
    second.channelCount = 1;
    (void)scene.addImage(second);

    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 16,
                            .imageHeight = 16,
                            .levelCount = 1};
    // Distinct payloads (base 0 vs base 170) so an image's data region being read from the
    // wrong chain index is detectable.
    PatternTileProvider provider0{layout, /*tileBytes=*/256, /*baseValue=*/0};
    PatternTileProvider provider1{layout, /*tileBytes=*/256, /*baseValue=*/170};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    std::vector<std::reference_wrapper<ptiff::io::TileProvider>> providers{std::ref(provider0),
                                                                           std::ref(provider1)};
    auto writeResult = (*writer)->write(scene, providers);
    REQUIRE(writeResult.has_value());

    auto reader = Reader::open(tmp.path());
    REQUIRE(reader.has_value());

    // The scene round-trips with both images.
    auto sceneResult = (*reader)->scene();
    REQUIRE(sceneResult.has_value());
    REQUIRE((*sceneResult)->imageCount().value() == 2);

    // Each image's pixels are reachable by its ImageId, in scene (chain) order, with distinct
    // payloads.
    auto pixels0 = readPixelsViaReader(tmp.path(), ImageId{0});
    REQUIRE(pixels0.has_value());
    const auto expected0 = expectedPattern(layout, 256, /*baseValue=*/0);
    REQUIRE(std::equal(pixels0->begin(), pixels0->end(), expected0.begin()));

    auto pixels1 = readPixelsViaReader(tmp.path(), ImageId{1});
    REQUIRE(pixels1.has_value());
    const auto expected1 = expectedPattern(layout, 256, /*baseValue=*/170);
    REQUIRE(std::equal(pixels1->begin(), pixels1->end(), expected1.begin()));
}

TEST_CASE("Reader::imageSource rejects an out-of-range image id", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(16, 16, 1, /*tiled=*/false);
    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 16,
                            .imageHeight = 16,
                            .levelCount = 1};
    PatternTileProvider provider{layout, /*tileBytes=*/256};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    auto writeResult = (*writer)->write(scene, provider);
    REQUIRE(writeResult.has_value());

    auto reader = Reader::open(tmp.path());
    REQUIRE(reader.has_value());
    auto source = (*reader)->imageSource(ImageId{7});
    REQUIRE_FALSE(source.has_value());
    REQUIRE(source.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("Reader::scene() still works alongside imageSource", "[read-pixels]") {
    TempFile tmp;
    Scene scene = buildScene(16, 16, 1, /*tiled=*/false);
    const TileLayout layout{.tileSize = {.width = 16, .height = 16},
                            .imageWidth = 16,
                            .imageHeight = 16,
                            .levelCount = 1};
    PatternTileProvider provider{layout, /*tileBytes=*/256};

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    auto writeResult = (*writer)->write(scene, provider);
    REQUIRE(writeResult.has_value());

    auto reader = Reader::open(tmp.path());
    REQUIRE(reader.has_value());

    // Metadata view still works after asking for pixels.
    auto sceneResult = (*reader)->scene();
    REQUIRE(sceneResult.has_value());
    REQUIRE((*sceneResult)->imageCount().value() == 1);

    auto source = (*reader)->imageSource(ImageId{0});
    REQUIRE(source.has_value());
    REQUIRE(source->get() != nullptr);
}
