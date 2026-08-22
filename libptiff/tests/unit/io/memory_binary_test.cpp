#include <array>
#include <cstddef>
#include <memory>
#include <vector>

#include <ptiff/io/memory_binary_reader.hpp>
#include <ptiff/io/memory_binary_writer.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::MemoryBinaryReader;
using ptiff::io::MemoryBinaryWriter;

namespace {

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

} // namespace

TEST_CASE("MemoryBinaryWriter writes sequentially and exposes the buffer", "[memory-binary]") {
    MemoryBinaryWriter w;
    REQUIRE(w.position().value() == 0);
    REQUIRE(w.buffer().empty());

    auto data = bytes({1, 2, 3, 4});
    REQUIRE(w.write(data).value() == 4);
    REQUIRE(w.position().value() == 4);
    REQUIRE(w.buffer() == data);
    REQUIRE(w.flush().has_value());

    // A second write appends.
    auto more = bytes({5});
    REQUIRE(w.write(more).value() == 1);
    REQUIRE(w.position().value() == 5);
    REQUIRE(w.buffer() == bytes({1, 2, 3, 4, 5}));
}

TEST_CASE("MemoryBinaryWriter seek overlays bytes in place", "[memory-binary]") {
    MemoryBinaryWriter w;
    REQUIRE(w.write(bytes({1, 2, 3, 4})).value() == 4);
    REQUIRE(w.seek(1).has_value());
    REQUIRE(w.position().value() == 1);
    REQUIRE(w.write(bytes({9, 9})).value() == 2);
    // Bytes at 0..4 are now 1, 9, 9, 4 (2 .. 3 overlaid with 9, 9? no: wrote two bytes at 1,2).
    REQUIRE(w.buffer() == bytes({1, 9, 9, 4}));
}

TEST_CASE("MemoryBinaryWriter seek beyond buffer is rejected", "[memory-binary]") {
    MemoryBinaryWriter w;
    REQUIRE(w.write(bytes({1, 2, 3})).value() == 3);
    auto seekResult = w.seek(5);
    REQUIRE_FALSE(seekResult.has_value());
    REQUIRE(seekResult.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("MemoryBinaryWriter takeBuffer moves bytes out and resets (zero position)",
          "[memory-binary]") {
    MemoryBinaryWriter w;
    REQUIRE(w.write(bytes({1, 2, 3})).value() == 3);
    auto moved = w.takeBuffer();
    REQUIRE(moved == bytes({1, 2, 3}));
    REQUIRE(w.buffer().empty());
    REQUIRE(w.position().value() == 0);
}

TEST_CASE("MemoryBinaryReader reads begin-to-end and shares buffer ownership", "[memory-binary]") {
    auto data = std::make_shared<const std::vector<std::byte>>(bytes({1, 2, 3, 4, 5}));
    MemoryBinaryReader r(data);
    REQUIRE(r.size().value() == 5);
    REQUIRE(r.position().value() == 0);

    std::array<std::byte, 3> chunk{};
    REQUIRE(r.read(chunk).value() == 3);
    REQUIRE(chunk == std::array{std::byte{1}, std::byte{2}, std::byte{3}});
    REQUIRE(r.position().value() == 3);

    std::array<std::byte, 4> rest{};
    REQUIRE(r.read(rest).value() == 2); // only 2 remain
    REQUIRE(rest[0] == std::byte{4});
    REQUIRE(rest[1] == std::byte{5});
    REQUIRE(r.position().value() == 5);
}

TEST_CASE("MemoryBinaryReader seek within buffer allows random access", "[memory-binary]") {
    auto data = std::make_shared<const std::vector<std::byte>>(bytes({7, 8, 9}));
    MemoryBinaryReader r(data);
    REQUIRE(r.seek(2).has_value());
    std::array<std::byte, 1> one{};
    REQUIRE(r.read(one).value() == 1);
    REQUIRE(one[0] == std::byte{9});
}

TEST_CASE("MemoryBinaryReader seek beyond buffer is rejected", "[memory-binary]") {
    auto data = std::make_shared<const std::vector<std::byte>>(bytes({1, 2, 3}));
    MemoryBinaryReader r(data);
    auto seekResult = r.seek(4);
    REQUIRE_FALSE(seekResult.has_value());
    REQUIRE(seekResult.error().code() == ptiff::ErrorCode::InvalidArgument);
}
