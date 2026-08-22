#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <utility>
#include <vector>

#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/memory_binary_reader.hpp>
#include <ptiff/io/memory_binary_writer.hpp>
#include <ptiff/io/scene_serializer.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile_provider.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::PixelType;
using ptiff::Scene;
using ptiff::io::MemoryBinaryReader;
using ptiff::io::MemoryBinaryWriter;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

// Deterministic provider: grid tile linear index i holds byte value (i % 256) repeated
// tileBytes times.
class PatternTileProvider final : public ptiff::io::TileProvider {
public:
    PatternTileProvider(TileLayout layout, std::uint32_t tileBytes)
        : layout_(layout),
          tiles_(layout_.columns() * layout_.rows()),
          tileBytes_(tileBytes),
          pixels_(std::size_t(tiles_) * tileBytes_, std::byte{0}) {
        for (std::uint32_t i = 0; i < tiles_; ++i) {
            const std::byte v = std::byte(static_cast<unsigned char>(i % 256));
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
            return std::unexpected(region.error());
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
                 std::optional<std::uint32_t> tileSize = std::nullopt) {
    Scene scene;
    ImageDescriptor gs;
    gs.width = width;
    gs.height = height;
    gs.pixelType = PixelType::UInt8;
    gs.channelCount = 1;
    gs.compression = CompressionKind::None;
    if (tileSize.has_value()) {
        gs.tileInfo = ptiff::TileInfo{.tileWidth = *tileSize, .tileHeight = *tileSize};
    }
    (void)scene.addImage(gs);
    return scene;
}

// Writes the scene's pixels to a MemoryBinaryWriter through the memory backend (+
// serializeModelList
// + openImageSinkAt), then reads them back via openImageSourceAt and returns the concatenated
// tile payloads in grid order (row-major). Errors propagate.
ptiff::Result<std::vector<std::byte>> memoryRoundTrip(const Scene& scene) {
    auto backend = ptiff::io::BackendFactory::instance().create("memory");
    if (!backend.has_value()) {
        return std::unexpected(backend.error());
    }

    ptiff::io::SceneSerializer serializer;
    auto model = serializer.serialize(scene);
    if (!model.has_value()) {
        return std::unexpected(model.error());
    }
    if (model->children().empty()) {
        return std::unexpected(
            ptiff::Error{ptiff::ErrorCode::InvalidArgument, "memoryRoundTrip: no images"});
    }
    const auto images = model->children(); // flat per-image models

    MemoryBinaryWriter writer;
    auto head = (*backend)->serializeModelList(images, writer);
    if (!head.has_value()) {
        return std::unexpected(head.error());
    }

    // One sink per image, fed from a matching provider.
    std::vector<std::shared_ptr<PatternTileProvider>> providers(images.size());
    std::vector<std::unique_ptr<ptiff::io::ImageSink>> sinks(images.size());
    for (std::size_t i = 0; i < images.size(); ++i) {
        auto sinkResult = (*backend)->openImageSinkAt(writer, images, i);
        if (!sinkResult.has_value()) {
            return std::unexpected(sinkResult.error());
        }
        std::unique_ptr<ptiff::io::ImageSink> sink = std::move(*sinkResult);
        const ptiff::io::tile::TileLayout& layout = sink->layout();
        // UInt8, 1 channel: tileBytes = tileW * tileH.
        providers[i] = std::make_shared<PatternTileProvider>(
            layout, layout.tileSize.width * layout.tileSize.height);

        for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
            for (std::uint32_t row = 0; row < layout.rows(level); ++row) {
                for (std::uint32_t col = 0; col < layout.columns(level); ++col) {
                    const TileIndex index{.column = col, .row = row, .level = level};
                    auto tile = providers[i]->provideTile(index);
                    if (!tile.has_value()) {
                        return std::unexpected(tile.error());
                    }
                    auto writeResult = sink->writeTile(*tile);
                    if (!writeResult.has_value()) {
                        return std::unexpected(writeResult.error());
                    }
                }
            }
        }
        sinks[i] = std::move(sink);
    }

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);

    std::vector<std::byte> out;
    for (std::size_t i = 0; i < images.size(); ++i) {
        auto sourceResult = (*backend)->openImageSourceAt(reader, i);
        if (!sourceResult.has_value()) {
            return std::unexpected(sourceResult.error());
        }
        const TileLayout& layout = (*sourceResult)->layout();
        for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
            for (std::uint32_t row = 0; row < layout.rows(level); ++row) {
                for (std::uint32_t col = 0; col < layout.columns(level); ++col) {
                    const TileIndex index{.column = col, .row = row, .level = level};
                    auto tile = (*sourceResult)->readTile(index);
                    if (!tile.has_value()) {
                        return std::unexpected(tile.error());
                    }
                    out.insert(out.end(), tile->data().begin(), tile->data().end());
                }
            }
        }
    }
    return out;
}

ptiff::Result<std::vector<std::byte>> readAllTiles(const Scene& scene) {
    // Build the same provider-driven tile stream the round-trip writes, to compare exactly.
    auto backend = ptiff::io::BackendFactory::instance().create("memory");
    if (!backend.has_value()) {
        return std::unexpected(backend.error());
    }
    ptiff::io::SceneSerializer serializer;
    auto model = serializer.serialize(scene);
    if (!model.has_value()) {
        return std::unexpected(model.error());
    }
    const auto images = model->children();

    std::vector<std::byte> out;
    for (const auto& image : images) {
        ptiff::io::tile::TileLayout layout{
            .tileSize = {.width = static_cast<std::uint32_t>(
                             std::stoul(image.field("tileWidth").value())),
                         .height = static_cast<std::uint32_t>(
                             std::stoul(image.field("tileHeight").value()))},
            .imageWidth = static_cast<std::uint32_t>(std::stoul(image.field("imageWidth").value())),
            .imageHeight =
                static_cast<std::uint32_t>(std::stoul(image.field("imageHeight").value())),
            .levelCount = 1};
        PatternTileProvider provider(layout, layout.tileSize.width * layout.tileSize.height);
        for (std::uint32_t row = 0; row < layout.rows(); ++row) {
            for (std::uint32_t col = 0; col < layout.columns(); ++col) {
                auto tile = provider.provideTile(TileIndex{.column = col, .row = row, .level = 0});
                out.insert(out.end(), tile->data().begin(), tile->data().end());
            }
        }
    }
    return out;
}

} // namespace

TEST_CASE("memory backend round-trips a single tiled image byte-for-byte", "[memory-pixels]") {
    auto scene = buildScene(16, 16, 16); // one 16x16 tile
    auto written = readAllTiles(scene);
    REQUIRE(written.has_value());
    auto read = memoryRoundTrip(scene);
    REQUIRE(read.has_value());
    REQUIRE(*read == *written);
    REQUIRE(read->size() > 0);
}

TEST_CASE("memory backend round-trips a 2x2 grid of 16px tiles", "[memory-pixels]") {
    auto scene = buildScene(32, 32, 16); // 2x2 tiles
    auto written = readAllTiles(scene);
    auto read = memoryRoundTrip(scene);
    REQUIRE(written.has_value());
    REQUIRE(read.has_value());
    REQUIRE(*read == *written);
}

TEST_CASE("memory backend round-trips a multi-image document", "[memory-pixels]") {
    Scene scene;
    {
        ImageDescriptor gs;
        gs.width = 16;
        gs.height = 16;
        gs.pixelType = PixelType::UInt8;
        gs.channelCount = 1;
        gs.compression = CompressionKind::None;
        gs.tileInfo = ptiff::TileInfo{.tileWidth = 8, .tileHeight = 8};
        (void)scene.addImage(gs);
    }
    {
        ImageDescriptor gs;
        gs.width = 32;
        gs.height = 16;
        gs.pixelType = PixelType::UInt8;
        gs.channelCount = 1;
        gs.compression = CompressionKind::None;
        gs.tileInfo = ptiff::TileInfo{.tileWidth = 16, .tileHeight = 16};
        (void)scene.addImage(gs);
    }

    auto written = readAllTiles(scene);
    auto read = memoryRoundTrip(scene);
    REQUIRE(written.has_value());
    REQUIRE(read.has_value());
    REQUIRE(*read == *written);
}
