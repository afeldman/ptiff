#include <cstddef>
#include <memory>
#include <string>
#include <vector>

#include <ptiff/io/backend/memory_backend.hpp>
#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/memory_binary_reader.hpp>
#include <ptiff/io/memory_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::MemoryBinaryReader;
using ptiff::io::MemoryBinaryWriter;
using ptiff::io::StorageModel;
using ptiff::io::backend::MemoryBackend;

namespace {

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

StorageModel makeImageModel() {
    StorageModel img;
    img.setField("imageWidth", "64");
    img.setField("imageHeight", "32");
    img.setField("tileWidth", "16");
    img.setField("tileHeight", "16");
    img.setField("samplesPerPixel", "1");
    img.setField("pixelType", "UInt8");
    img.setField("compression", "None");
    return img;
}

StorageModel makeSceneRoot() {
    StorageModel root;
    root.addChild(makeImageModel());
    return root;
}

} // namespace

TEST_CASE("MemoryBackend::name and capabilities", "[memory-backend]") {
    MemoryBackend backend;
    REQUIRE(backend.name() == "memory");
    auto caps = backend.capabilities();
    REQUIRE(caps.supportsTiling);
    REQUIRE_FALSE(caps.supportsStreaming);
    REQUIRE(caps.supportsRandomAccess);
    REQUIRE_FALSE(caps.supportsCloudStreaming);
}

TEST_CASE("BackendFactory::create(\"memory\") returns the registered backend", "[memory-backend]") {
    auto backend = ptiff::io::BackendFactory::instance().create("memory");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == "memory");
}

TEST_CASE("serializeModel -> deserializeModel round-trips the model tree", "[memory-backend]") {
    MemoryBackend backend;
    auto root = makeSceneRoot(); // root with one child (the "full" scene shape)

    MemoryBinaryWriter writer;
    REQUIRE(backend.serializeModel(root, writer).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto parsed = backend.deserializeModel(reader);
    REQUIRE(parsed.has_value());
    REQUIRE(parsed->children().size() == 1);
    REQUIRE(parsed->children()[0].field("imageWidth").value() == "64");
    REQUIRE(parsed->children()[0].field("pixelType").value() == "UInt8");
}

TEST_CASE("serializeModelList writes a multi-image document readable by deserializeModel",
          "[memory-backend]") {
    MemoryBackend backend;
    StorageModel a = makeImageModel();
    auto sb = makeImageModel();
    sb.setField("imageHeight", "16");
    std::vector<StorageModel> models;
    models.push_back(std::move(a));
    models.push_back(std::move(sb));

    MemoryBinaryWriter writer;
    REQUIRE(backend.serializeModelList(models, writer).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto parsed = backend.deserializeModel(reader);
    REQUIRE(parsed.has_value());
    REQUIRE(parsed->children().size() == 2);
    REQUIRE(parsed->children()[0].field("imageHeight").value() == "32");
    REQUIRE(parsed->children()[1].field("imageHeight").value() == "16");
}

TEST_CASE("deserializeModel rejects bad magic", "[memory-backend]") {
    MemoryBackend backend;
    // Wrong first byte.
    std::vector<std::byte> bad =
        bytes({'X', 'M', 'E', 'M', 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00});
    auto data = std::make_shared<const std::vector<std::byte>>(bad);
    MemoryBinaryReader reader(data);
    auto parsed = backend.deserializeModel(reader);
    REQUIRE_FALSE(parsed.has_value());
    REQUIRE(parsed.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("deserializeModel rejects unsupported version", "[memory-backend]") {
    MemoryBackend backend;
    // magic ok, version u16 = 99.
    std::vector<std::byte> bad =
        bytes({'P', 'M', 'E', 'M', 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00});
    auto data = std::make_shared<const std::vector<std::byte>>(bad);
    MemoryBinaryReader reader(data);
    auto parsed = backend.deserializeModel(reader);
    REQUIRE_FALSE(parsed.has_value());
    REQUIRE(parsed.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("openImageSourceAt rejects an out-of-range image index", "[memory-backend]") {
    MemoryBackend backend;
    auto root = makeSceneRoot(); // 1 child
    MemoryBinaryWriter writer;
    REQUIRE(backend.serializeModel(root, writer).has_value());

    auto data = std::make_shared<const std::vector<std::byte>>(writer.takeBuffer());
    MemoryBinaryReader reader(data);
    auto source = backend.openImageSourceAt(reader, 5);
    REQUIRE_FALSE(source.has_value());
    REQUIRE(source.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("openImageSinkAt rejects an out-of-range image index", "[memory-backend]") {
    MemoryBackend backend;
    auto a = makeImageModel();
    std::vector<StorageModel> models;
    models.push_back(std::move(a));

    MemoryBinaryWriter writer;
    auto sink = backend.openImageSinkAt(writer, models, 3);
    REQUIRE_FALSE(sink.has_value());
    REQUIRE(sink.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("imageInfoFromModel rejects a model missing tileWidth", "[memory-backend]") {
    MemoryBackend backend;
    StorageModel img;
    img.setField("imageWidth", "64");
    img.setField("imageHeight", "32");
    img.setField("samplesPerPixel", "1");
    img.setField("pixelType", "UInt8");

    // openImageSinkAt → imageInfos → imageInfoFromModel should reject the missing tileWidth.
    std::vector<StorageModel> models;
    models.push_back(std::move(img));
    MemoryBinaryWriter writer;
    auto sink = backend.openImageSinkAt(writer, models, 0);
    REQUIRE_FALSE(sink.has_value());
    REQUIRE(sink.error().code() == ptiff::ErrorCode::InvalidArgument);
}
