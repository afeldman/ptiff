#include <cstdint>
#include <string>
#include <vector>

#include <ptiff/io/backend/memory/memory_codec.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::memory::memoryCodecByteSize;
using ptiff::io::backend::memory::memoryDecode;
using ptiff::io::backend::memory::memoryEncode;
using ptiff::io::backend::memory::memoryEncodeDocument;

namespace {

StorageModel makeImage(std::uint32_t w, std::uint32_t h) {
    StorageModel img;
    img.setField("imageWidth", std::to_string(w));
    img.setField("imageHeight", std::to_string(h));
    img.setField("samplesPerPixel", "1");
    img.setField("pixelType", "UInt16");
    return img;
}

} // namespace

TEST_CASE("codec round-trips a single node with fields", "[memory-codec]") {
    StorageModel node;
    node.setField("b", "2");
    node.setField("a", "1");
    node.setField("model", "HiRISE");

    std::vector<std::byte> out;
    std::uint64_t budget = 0;
    REQUIRE(memoryEncode(out, node, budget).has_value());
    // byteSize agrees with the written length.
    REQUIRE(out.size() == memoryCodecByteSize(node));

    std::size_t cursor = 0;
    budget = 0;
    auto decoded = memoryDecode(out, cursor, budget);
    REQUIRE(decoded.has_value());
    REQUIRE(decoded->field("a").value() == "1");
    REQUIRE(decoded->field("b").value() == "2");
    REQUIRE(decoded->field("model").value() == "HiRISE");
    REQUIRE(decoded->children().empty());
    REQUIRE(cursor == out.size());
}

TEST_CASE("codec round-trips a root with children (scene shape)", "[memory-codec]") {
    StorageModel root;
    root.addChild(makeImage(64, 32));
    root.addChild(makeImage(10, 10));

    std::vector<std::byte> out;
    std::uint64_t budget = 0;
    REQUIRE(memoryEncode(out, root, budget).has_value());
    REQUIRE(out.size() == memoryCodecByteSize(root));

    std::size_t cursor = 0;
    budget = 0;
    auto decoded = memoryDecode(out, cursor, budget);
    REQUIRE(decoded.has_value());
    REQUIRE(decoded->children().size() == 2);
    REQUIRE(decoded->children()[0].field("imageWidth").value() == "64");
    REQUIRE(decoded->children()[1].field("imageWidth").value() == "10");
    REQUIRE(cursor == out.size());
}

TEST_CASE("codec byteSize is order-independent for fields and children", "[memory-codec]") {
    StorageModel a;
    a.setField("z", "last");
    a.setField("a", "first");
    a.addChild(makeImage(1, 1));
    a.addChild(makeImage(2, 2));

    StorageModel b;
    b.setField("a", "first");
    b.setField("z", "last");
    b.addChild(makeImage(2, 2));
    b.addChild(makeImage(1, 1));

    // Sorted field order + same multiset of children => identical byte size.
    REQUIRE(memoryCodecByteSize(a) == memoryCodecByteSize(b));
}

TEST_CASE("memoryEncodeDocument wraps a sibling model list under a fieldless root",
          "[memory-codec]") {
    StorageModel a = makeImage(4, 4);
    StorageModel b = makeImage(8, 8);
    std::vector<StorageModel> models;
    models.push_back(std::move(a));
    models.push_back(std::move(b));

    std::vector<std::byte> out;
    std::uint64_t budget = 0;
    REQUIRE(memoryEncodeDocument(out, models, budget).has_value());
    REQUIRE(out.size() == ptiff::io::backend::memory::memoryDocumentByteSize(models));

    std::size_t cursor = 0;
    budget = 0;
    auto decoded = memoryDecode(out, cursor, budget);
    REQUIRE(decoded.has_value());
    REQUIRE(decoded->children().size() == 2);
    REQUIRE(decoded->field("imageWidth").has_value() == false); // root is fieldless
    REQUIRE(cursor == out.size());
}

TEST_CASE("codec rejects truncated input", "[memory-codec]") {
    // Build a valid document, then cut it short.
    StorageModel root;
    root.addChild(makeImage(4, 4));
    std::vector<std::byte> out;
    std::uint64_t budget = 0;
    REQUIRE(memoryEncode(out, root, budget).has_value());

    out.resize(out.size() - 3); // truncate the tail
    std::size_t cursor = 0;
    budget = 0;
    auto decoded = memoryDecode(out, cursor, budget);
    REQUIRE_FALSE(decoded.has_value());
    REQUIRE(decoded.error().code() == ptiff::ErrorCode::InvalidArgument);
}
