#include <cstddef>
#include <cstdint>
#include <cstring>
#include <memory>
#include <span>
#include <vector>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/memory_binary_reader.hpp>
#include <ptiff/io/memory_binary_writer.hpp>
#include <ptiff/io/storage_backend.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::MemoryBinaryReader;
using ptiff::io::MemoryBinaryWriter;
using ptiff::io::StorageModel;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;

namespace {

StorageModel makeImageModel(std::uint32_t width,
                            std::uint32_t height,
                            std::uint32_t spp,
                            const char* pixelType) {
    StorageModel img;
    img.setField("imageWidth", std::to_string(width));
    img.setField("imageHeight", std::to_string(height));
    img.setField("samplesPerPixel", std::to_string(spp));
    img.setField("pixelType", pixelType);
    return img;
}

using BackendPtr = std::unique_ptr<ptiff::io::StorageBackend>;
BackendPtr makeBackend() {
    auto backend = ptiff::io::BackendFactory::instance().create("openexr");
    REQUIRE(backend.has_value());
    return std::move(*backend);
}

// Writes `bytes` (spanning the whole image) through the backend's single-tile sink, then reads
// them back through its single-tile source, and returns the round-tripped bytes.
ptiff::Result<std::vector<std::byte>> roundTrip(ptiff::io::StorageBackend& backend,
                                                const StorageModel& model,
                                                std::span<const std::byte> bytes) {
    MemoryBinaryWriter writer;
    auto sinkResult = backend.openImageSink(writer, model);
    if (!sinkResult.has_value()) {
        return std::unexpected(sinkResult.error());
    }
    const TileIndex index{.column = 0, .row = 0, .level = 0};
    auto region = (*sinkResult)->layout().regionFor(index);
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }
    Tile tile(ptiff::TileId{0}, index, *region, bytes);
    auto written = (*sinkResult)->writeTile(tile);
    if (!written.has_value()) {
        return std::unexpected(written.error());
    }

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto sourceResult = backend.openImageSource(reader);
    if (!sourceResult.has_value()) {
        return std::unexpected(sourceResult.error());
    }
    auto readBack = (*sourceResult)->readTile(index);
    if (!readBack.has_value()) {
        return std::unexpected(readBack.error());
    }
    return std::vector<std::byte>{readBack->data().begin(), readBack->data().end()};
}

} // namespace

TEST_CASE("OpenExrBackend::name and capabilities", "[openexr-backend]") {
    const auto caps = makeBackend()->capabilities();
    REQUIRE(caps.supportsRandomAccess);
}

TEST_CASE("BackendFactory::create(\"openexr\") returns the registered backend",
          "[openexr-backend]") {
    auto backend = ptiff::io::BackendFactory::instance().create("openexr");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == "openexr");
}

TEST_CASE("OpenEXR serializeModel -> deserializeModel round-trips the image model",
          "[openexr-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(4, 3, 1, "Float32");

    MemoryBinaryWriter writer;
    REQUIRE(backend->serializeModel(model, writer).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto parsed = backend->deserializeModel(reader);
    REQUIRE(parsed.has_value());
    REQUIRE(parsed->children().size() == 1);
    REQUIRE(parsed->children()[0].field("imageWidth").value() == "4");
    REQUIRE(parsed->children()[0].field("imageHeight").value() == "3");
    REQUIRE(parsed->children()[0].field("samplesPerPixel").value() == "1");
    REQUIRE(parsed->children()[0].field("pixelType").value() == "Float32");
}

TEST_CASE("OpenEXR backend round-trips a grayscale Float32 image byte-for-byte",
          "[openexr-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(4, 3, 1, "Float32");

    std::vector<float> pixels(4 * 3);
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        pixels[i] = static_cast<float>(i) + 0.5f;
    }
    std::vector<std::byte> bytes(pixels.size() * sizeof(float));
    std::memcpy(bytes.data(), pixels.data(), bytes.size());

    auto readBack = roundTrip(*backend, model, bytes);
    REQUIRE(readBack.has_value());
    REQUIRE(readBack->size() == bytes.size());
    REQUIRE(std::memcmp(readBack->data(), bytes.data(), bytes.size()) == 0);
}

TEST_CASE("OpenEXR backend round-trips an RGB UInt32 image byte-for-byte", "[openexr-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(2, 2, 3, "UInt32");

    std::vector<std::uint32_t> pixels(2 * 2 * 3);
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        pixels[i] = static_cast<std::uint32_t>(i * 7 + 1);
    }
    std::vector<std::byte> bytes(pixels.size() * sizeof(std::uint32_t));
    std::memcpy(bytes.data(), pixels.data(), bytes.size());

    auto readBack = roundTrip(*backend, model, bytes);
    REQUIRE(readBack.has_value());
    REQUIRE(readBack->size() == bytes.size());
    REQUIRE(std::memcmp(readBack->data(), bytes.data(), bytes.size()) == 0);
}

TEST_CASE("OpenEXR backend rejects an unsupported pixel type", "[openexr-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(2, 2, 1, "UInt8");
    MemoryBinaryWriter writer;
    auto result = backend->openImageSink(writer, model);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("OpenEXR backend rejects an unsupported samplesPerPixel", "[openexr-backend]") {
    auto backend = makeBackend();
    auto model = makeImageModel(2, 2, 2, "Float32");
    MemoryBinaryWriter writer;
    auto result = backend->openImageSink(writer, model);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
