#include <cstddef>
#include <vector>

#include <ptiff/compression/predictor.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::compression::undoHorizontalDifferencing;

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

TEST_CASE("undoHorizontalDifferencing reverses an 8-bit single-sample row", "[predictor]") {
    // Row: first value 10, then differences +10, +0 -> running sum: 10, 20, 20.
    auto data = bytes({10, 10, 0});
    auto result = undoHorizontalDifferencing(data,
                                             /*rowWidth=*/3,
                                             /*samplesPerPixel=*/1,
                                             /*bytesPerSample=*/1,
                                             /*bigEndian=*/false);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({10, 20, 20}));
}

TEST_CASE("undoHorizontalDifferencing wraps modulo 256 for 8-bit samples", "[predictor]") {
    // Row: first value 200, then difference +100 -> running sum wraps: (200+100)&0xFF = 44.
    auto data = bytes({200, 100});
    auto result = undoHorizontalDifferencing(data, 2, 1, 1, false);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({200, 44}));
}

TEST_CASE("undoHorizontalDifferencing resets the running sum at each row boundary", "[predictor]") {
    // Two rows of width 2: row0 {10,10}->{10,20}; row1 {5,5}->{5,10} (independent of row0).
    auto data = bytes({10, 10, 5, 5});
    auto result = undoHorizontalDifferencing(data, 2, 1, 1, false);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({10, 20, 5, 10}));
}

TEST_CASE("undoHorizontalDifferencing interleaves multiple sample components independently",
          "[predictor]") {
    // 2 columns, 2 samples/pixel (e.g. RGB-like with 2 channels): column0=(10,20),
    // column1 diffs=(+5,+5) -> running=(15,25). Byte layout is interleaved: R0 G0 R1 G1.
    auto data = bytes({10, 20, 5, 5});
    auto result = undoHorizontalDifferencing(data, 2, 2, 1, false);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({10, 20, 15, 25}));
}

TEST_CASE("undoHorizontalDifferencing handles 16-bit little-endian samples", "[predictor]") {
    // Column0 = 0x0100 (256, LE bytes {0x00,0x01}), column1 diff = 0x0001 (1, LE {0x01,0x00})
    // -> running = 257 = 0x0101, LE bytes {0x01,0x01}.
    auto data = bytes({0x00, 0x01, 0x01, 0x00});
    auto result = undoHorizontalDifferencing(data, 2, 1, 2, /*bigEndian=*/false);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({0x00, 0x01, 0x01, 0x01}));
}

TEST_CASE("undoHorizontalDifferencing handles 16-bit big-endian samples", "[predictor]") {
    // Same values as the little-endian case, but byte order flipped: column0=0x0100 stored as
    // {0x01,0x00}; column1 diff=0x0001 stored as {0x00,0x01} -> running=0x0101 stored {0x01,0x01}.
    auto data = bytes({0x01, 0x00, 0x00, 0x01});
    auto result = undoHorizontalDifferencing(data, 2, 1, 2, /*bigEndian=*/true);
    REQUIRE(result.has_value());
    REQUIRE(data == bytes({0x01, 0x00, 0x01, 0x01}));
}

TEST_CASE("undoHorizontalDifferencing rejects an unsupported bytesPerSample", "[predictor]") {
    auto data = bytes({1, 2, 3});
    auto result = undoHorizontalDifferencing(data, 3, 1, 3, false);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("undoHorizontalDifferencing rejects a size that isn't a multiple of the row stride",
          "[predictor]") {
    auto data = bytes({1, 2, 3}); // rowWidth=2, samplesPerPixel=1, bytesPerSample=1 -> stride 2,
                                  // but data.size()=3 is not a multiple of 2.
    auto result = undoHorizontalDifferencing(data, 2, 1, 1, false);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("applyHorizontalDifferencing round-trips undoHorizontalDifferencing",
          "[compression-predictor]") {
    // 3x2 grayscale UInt8: row0 {10,20,30} row1 {40,50,60}
    std::vector<std::byte> data{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    auto encoded = data;
    auto dirResult = ptiff::compression::applyHorizontalDifferencing(encoded, 3, 1, 1, false);
    REQUIRE(dirResult.has_value());
    // After differencing row0 = {10,10,10} row1 = {40,10,10}
    REQUIRE(std::to_integer<std::uint8_t>(encoded[0]) == 10);
    REQUIRE(std::to_integer<std::uint8_t>(encoded[1]) == 10);
    REQUIRE(std::to_integer<std::uint8_t>(encoded[2]) == 10);
    REQUIRE(std::to_integer<std::uint8_t>(encoded[3]) == 40);
    REQUIRE(std::to_integer<std::uint8_t>(encoded[4]) == 10);
    REQUIRE(std::to_integer<std::uint8_t>(encoded[5]) == 10);

    auto decoded = encoded;
    auto undirResult = ptiff::compression::undoHorizontalDifferencing(decoded, 3, 1, 1, false);
    REQUIRE(undirResult.has_value());
    REQUIRE(decoded == data);
}
