#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <string>
#include <vector>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/memory_binary_reader.hpp>
#include <ptiff/io/memory_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile_provider.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::MemoryBinaryReader;
using ptiff::io::MemoryBinaryWriter;
using ptiff::io::StorageModel;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

StorageModel makeImageModel() {
    StorageModel img;
    img.setField("imageWidth", "32");
    img.setField("imageHeight", "32");
    img.setField("tileWidth", "16");
    img.setField("tileHeight", "16");
    img.setField("samplesPerPixel", "1");
    img.setField("pixelType", "UInt8");
    img.setField("compression", "None");
    return img;
}

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
        const std::uint64_t linear =
            static_cast<std::uint64_t>(index.row) * layout_.columns(index.level) + index.column;
        return Tile{ptiff::TileId(linear),
                    index,
                    *region,
                    std::span<const std::byte>{pixels_}.subspan(
                        static_cast<std::size_t>(linear) * tileBytes_, tileBytes_)};
    }

private:
    TileLayout layout_;
    std::uint32_t tiles_;
    std::uint32_t tileBytes_;
    std::vector<std::byte> pixels_;
};

std::unique_ptr<ptiff::io::StorageBackend> makeBackend() {
    auto backend = ptiff::io::BackendFactory::instance().create("isis");
    REQUIRE(backend.has_value());
    return std::move(*backend);
}

ptiff::Result<void> writePixels(ptiff::io::StorageBackend& backend,
                                MemoryBinaryWriter& writer,
                                const StorageModel& model) {
    auto sinkResult = backend.openImageSink(writer, model);
    if (!sinkResult.has_value()) {
        return std::unexpected(sinkResult.error());
    }
    const TileLayout& layout = (*sinkResult)->layout();
    PatternTileProvider provider(layout, layout.tileSize.width * layout.tileSize.height);
    for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
        for (std::uint32_t row = 0; row < layout.rows(level); ++row) {
            for (std::uint32_t col = 0; col < layout.columns(level); ++col) {
                const TileIndex index{.column = col, .row = row, .level = level};
                auto tile = provider.provideTile(index);
                if (!tile.has_value()) {
                    return std::unexpected(tile.error());
                }
                auto w = (*sinkResult)->writeTile(*tile);
                if (!w.has_value()) {
                    return std::unexpected(w.error());
                }
            }
        }
    }
    return {};
}

ptiff::Result<std::vector<std::byte>> readPixels(ptiff::io::StorageBackend& backend,
                                                 MemoryBinaryReader& reader) {
    auto sourceResult = backend.openImageSource(reader);
    if (!sourceResult.has_value()) {
        return std::unexpected(sourceResult.error());
    }
    const TileLayout& layout = (*sourceResult)->layout();
    std::vector<std::byte> out;
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
    return out;
}

} // namespace

TEST_CASE("IsisBackend::name and capabilities", "[isis-backend]") {
    auto backend = makeBackend();
    const auto caps = backend->capabilities();
    REQUIRE(caps.supportsTiling);
    REQUIRE_FALSE(caps.supportsStreaming);
    REQUIRE(caps.supportsRandomAccess);
    REQUIRE_FALSE(caps.supportsCloudStreaming);
}

TEST_CASE("BackendFactory::create(\"isis\") returns the registered backend", "[isis-backend]") {
    auto backend = ptiff::io::BackendFactory::instance().create("isis");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == "isis");
}

TEST_CASE("ISIS serializeModel -> deserializeModel round-trips the image model", "[isis-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel();

    MemoryBinaryWriter writer;
    REQUIRE(backend->serializeModel(model, writer).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto parsed = backend->deserializeModel(reader);
    REQUIRE(parsed.has_value());
    REQUIRE(parsed->children().size() == 1);
    REQUIRE(parsed->children()[0].field("imageWidth").value() == "32");
    REQUIRE(parsed->children()[0].field("imageHeight").value() == "32");
    REQUIRE(parsed->children()[0].field("tileWidth").value() == "16");
    REQUIRE(parsed->children()[0].field("pixelType").value() == "UInt8");
}

TEST_CASE("ISIS backend round-trips a 2x2 grid of tiles byte-for-byte", "[isis-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(); // 32x32, 16x16 tiles -> 2x2 grid

    MemoryBinaryWriter writer;
    REQUIRE(backend->serializeModel(model, writer).has_value());
    REQUIRE(writePixels(*backend, writer, model).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto read = readPixels(*backend, reader);
    REQUIRE(read.has_value());
    REQUIRE(read->size() == 32u * 32u);
    REQUIRE((*read)[0] == std::byte{0});
    REQUIRE((*read)[16 * 16] == std::byte{1});
    REQUIRE((*read)[2 * 16 * 16] == std::byte{2});
}
