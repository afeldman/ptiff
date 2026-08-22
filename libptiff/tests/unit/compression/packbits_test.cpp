#include <cstddef>
#include <vector>

#include <ptiff/compression/packbits.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::compression::decodePackBits;

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

TEST_CASE("decodePackBits decodes a literal run", "[packbits]") {
    // control=2 (non-negative) -> copy the next 2+1=3 literal bytes verbatim.
    auto input = bytes({0x02, 0x01, 0x02, 0x03});
    auto result = decodePackBits(input, 3);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x01, 0x02, 0x03}));
}

TEST_CASE("decodePackBits decodes a repeat run", "[packbits]") {
    // control=-4 (0xFC as a signed byte) -> repeat the next byte (1-(-4))=5 times.
    auto input = bytes({0xFC, 0x09});
    auto result = decodePackBits(input, 5);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x09, 0x09, 0x09, 0x09, 0x09}));
}

TEST_CASE("decodePackBits treats control byte -128 as a no-op", "[packbits]") {
    // 0x80 (-128) consumes only itself and produces no output, then a literal run follows.
    auto input = bytes({0x80, 0x02, 0x01, 0x02, 0x03});
    auto result = decodePackBits(input, 3);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x01, 0x02, 0x03}));
}

TEST_CASE("decodePackBits decodes a hand-verified mixed literal+repeat stream", "[packbits]") {
    // control=2 -> literal {0x01,0x02,0x03}; control=-4 (0xFC) -> repeat 0x09 five times.
    auto input = bytes({0x02, 0x01, 0x02, 0x03, 0xFC, 0x09});
    auto result = decodePackBits(input, 8);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x01, 0x02, 0x03, 0x09, 0x09, 0x09, 0x09, 0x09}));
}

TEST_CASE("decodePackBits rejects a literal run that reads past the input's end", "[packbits]") {
    // control=2 asks for 3 literal bytes, but only 2 remain in the input.
    auto input = bytes({0x02, 0x01, 0x02});
    auto result = decodePackBits(input, 3);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodePackBits rejects a repeat run missing its value byte", "[packbits]") {
    auto input = bytes({0xFC}); // control=-4, but no value byte follows
    auto result = decodePackBits(input, 5);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodePackBits rejects input exhausted before expectedSize is reached", "[packbits]") {
    auto input = bytes({0x02, 0x01, 0x02, 0x03}); // decodes to exactly 3 bytes
    auto result = decodePackBits(input, 5);       // but caller expects 5
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodePackBits rejects a stream that would decode past expectedSize", "[packbits]") {
    // control=-4 (0xFC) asks for a 5-byte repeat run, but expectedSize is only 3 -- this is the
    // decompression-bomb guard: fail the instant the bound would be exceeded.
    auto input = bytes({0xFC, 0x09});
    auto result = decodePackBits(input, 3);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("encodePackBits round-trips decodePackBits on repetitive data",
          "[compression-packbits]") {
    std::vector<std::byte> data(64, std::byte{0xAB});
    auto enc = ptiff::compression::encodePackBits(data);
    REQUIRE(enc.has_value());
    auto dec = ptiff::compression::decodePackBits(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}

TEST_CASE("encodePackBits round-trips decodePackBits on changing data", "[compression-packbits]") {
    std::vector<std::byte> data(32);
    for (int i = 0; i < 32; ++i)
        data[static_cast<std::size_t>(i)] = static_cast<std::byte>(i);
    auto enc = ptiff::compression::encodePackBits(data);
    REQUIRE(enc.has_value());
    auto dec = ptiff::compression::decodePackBits(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}
