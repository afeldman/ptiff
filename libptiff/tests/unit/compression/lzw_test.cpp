#include <cstddef>
#include <vector>

#include <ptiff/compression/lzw.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::compression::decodeLzw;

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

TEST_CASE("decodeLzw decodes two raw-code bytes with no dictionary reuse", "[lzw]") {
    // Codes: Clear(256), 'A'(65), 'B'(66), EOI(257) -- all 9-bit, MSB-first packed.
    auto input = bytes({0x80, 0x10, 0x48, 0x50, 0x10});
    auto result = decodeLzw(input, 2);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x41, 0x42}));
}

TEST_CASE("decodeLzw decodes a stream that reuses a dictionary entry", "[lzw]") {
    // Codes: Clear(256), 'A'(65), 'B'(66) [creates dict[258]="AB"], 258 ("AB" backreference),
    // EOI(257). Decodes to "ABAB".
    auto input = bytes({0x80, 0x10, 0x48, 0x50, 0x28, 0x08});
    auto result = decodeLzw(input, 4);
    REQUIRE(result.has_value());
    REQUIRE(*result == bytes({0x41, 0x42, 0x41, 0x42}));
}

TEST_CASE("decodeLzw rejects a code referencing an undefined dictionary entry", "[lzw]") {
    // Codes: Clear(256), 258 -- 258 isn't valid immediately after Clear (no prior code to extend).
    auto input = bytes({0x80, 0x40, 0x80});
    auto result = decodeLzw(input, 2);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeLzw rejects a stream that would decode past expectedSize", "[lzw]") {
    // Same "AB" stream as above, but expectedSize=1 -- decoding 'A' hits the bound exactly, then
    // decoding 'B' would exceed it. This is the decompression-bomb guard.
    auto input = bytes({0x80, 0x10, 0x48, 0x50, 0x10});
    auto result = decodeLzw(input, 1);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeLzw rejects a bitstream that runs out before EOI", "[lzw]") {
    auto input = bytes({0x80, 0x10}); // Clear, 'A', then nothing -- no EOI
    auto result = decodeLzw(input, 1);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeLzw rejects output shorter than expectedSize", "[lzw]") {
    // Decodes to exactly 2 bytes ("AB") but the caller expects 4.
    auto input = bytes({0x80, 0x10, 0x48, 0x50, 0x10});
    auto result = decodeLzw(input, 4);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("encodeLzw round-trips decodeLzw on a small grayscale row", "[compression-lzw]") {
    std::vector<std::byte> data{std::byte{10},
                                std::byte{10},
                                std::byte{10},
                                std::byte{20},
                                std::byte{20},
                                std::byte{30},
                                std::byte{31},
                                std::byte{30}};
    auto enc = ptiff::compression::encodeLzw(data);
    REQUIRE(enc.has_value());
    REQUIRE(enc->size() >= 4); // at least clear + min literals + EOI
    auto dec = ptiff::compression::decodeLzw(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}

TEST_CASE("encodeLzw round-trips decodeLzw across the code-width boundary", "[compression-lzw]") {
    // Regression test for the encoder's code-width-schedule sync at the TIFF early-change
    // boundaries (511/1023/2047). Build input large enough that the encoder's dictionary grows
    // well past 2047 so the 9->10->11->12 width transitions are all exercised; a miscount that
    // shifts a width change one code early desynchronizes the decoder and makes it reject the
    // stream (invalid code / dictionary out of range).
    std::vector<std::byte> data;
    data.reserve(4096);
    std::uint32_t seed = 0xA5A5A5A5U;
    for (std::size_t i = 0; i < 4096; ++i) {
        // Deterministic pseudo-random bytes: varied so LZW keeps discovering new phrases and the
        // dictionary grows past 511/1023/2047.
        seed = seed * 1664525U + 1013904223U;
        data.push_back(static_cast<std::byte>((seed >> 24) & 0xFFU));
    }
    auto enc = ptiff::compression::encodeLzw(data);
    REQUIRE(enc.has_value());
    REQUIRE(enc->size() > 0);
    auto dec = ptiff::compression::decodeLzw(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}
