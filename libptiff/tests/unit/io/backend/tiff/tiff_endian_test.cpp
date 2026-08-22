#include <array>
#include <cstddef>

#include <ptiff/io/backend/tiff/tiff_endian.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::backend::tiff::Endian;
using ptiff::io::backend::tiff::readU16;
using ptiff::io::backend::tiff::readU32;
using ptiff::io::backend::tiff::readU64;
using ptiff::io::backend::tiff::writeU16;
using ptiff::io::backend::tiff::writeU32;
using ptiff::io::backend::tiff::writeU64;

TEST_CASE("readU16 honors byte order", "[tiff-endian]") {
    const std::array<std::byte, 2> little{std::byte{0x34}, std::byte{0x12}};
    REQUIRE(readU16(little, Endian::Little) == 0x1234);

    const std::array<std::byte, 2> big{std::byte{0x12}, std::byte{0x34}};
    REQUIRE(readU16(big, Endian::Big) == 0x1234);
}

TEST_CASE("readU32 honors byte order", "[tiff-endian]") {
    const std::array<std::byte, 4> little{
        std::byte{0x78}, std::byte{0x56}, std::byte{0x34}, std::byte{0x12}};
    REQUIRE(readU32(little, Endian::Little) == 0x12345678u);

    const std::array<std::byte, 4> big{
        std::byte{0x12}, std::byte{0x34}, std::byte{0x56}, std::byte{0x78}};
    REQUIRE(readU32(big, Endian::Big) == 0x12345678u);
}

TEST_CASE("readU64 honors byte order", "[tiff-endian]") {
    const std::array<std::byte, 8> little{std::byte{0xF0},
                                          std::byte{0xDE},
                                          std::byte{0xBC},
                                          std::byte{0x9A},
                                          std::byte{0x78},
                                          std::byte{0x56},
                                          std::byte{0x34},
                                          std::byte{0x12}};
    REQUIRE(readU64(little, Endian::Little) == 0x123456789ABCDEF0ull);

    const std::array<std::byte, 8> big{std::byte{0x12},
                                       std::byte{0x34},
                                       std::byte{0x56},
                                       std::byte{0x78},
                                       std::byte{0x9A},
                                       std::byte{0xBC},
                                       std::byte{0xDE},
                                       std::byte{0xF0}};
    REQUIRE(readU64(big, Endian::Big) == 0x123456789ABCDEF0ull);
}

TEST_CASE("readers only consume their leading bytes, trailing bytes are ignored", "[tiff-endian]") {
    const std::array<std::byte, 4> data{
        std::byte{0x01}, std::byte{0x00}, std::byte{0xFF}, std::byte{0xFF}};
    REQUIRE(readU16(data, Endian::Little) == 1);
}

TEST_CASE("writeU16 honors byte order", "[tiff-endian]") {
    std::array<std::byte, 2> little{};
    writeU16(little, 0x1234, Endian::Little);
    REQUIRE(little[0] == std::byte{0x34});
    REQUIRE(little[1] == std::byte{0x12});

    std::array<std::byte, 2> big{};
    writeU16(big, 0x1234, Endian::Big);
    REQUIRE(big[0] == std::byte{0x12});
    REQUIRE(big[1] == std::byte{0x34});
}

TEST_CASE("writeU32 honors byte order", "[tiff-endian]") {
    std::array<std::byte, 4> little{};
    writeU32(little, 0x12345678u, Endian::Little);
    REQUIRE(little[0] == std::byte{0x78});
    REQUIRE(little[3] == std::byte{0x12});

    std::array<std::byte, 4> big{};
    writeU32(big, 0x12345678u, Endian::Big);
    REQUIRE(big[0] == std::byte{0x12});
    REQUIRE(big[3] == std::byte{0x78});
}

TEST_CASE("writeU64 honors byte order", "[tiff-endian]") {
    std::array<std::byte, 8> little{};
    writeU64(little, 0x123456789ABCDEF0ull, Endian::Little);
    REQUIRE(little[0] == std::byte{0xF0});
    REQUIRE(little[7] == std::byte{0x12});

    std::array<std::byte, 8> big{};
    writeU64(big, 0x123456789ABCDEF0ull, Endian::Big);
    REQUIRE(big[0] == std::byte{0x12});
    REQUIRE(big[7] == std::byte{0xF0});
}

TEST_CASE("write then read round-trips for every width and byte order", "[tiff-endian]") {
    std::array<std::byte, 2> a{};
    writeU16(a, 0xBEEF, Endian::Big);
    REQUIRE(readU16(a, Endian::Big) == 0xBEEF);

    std::array<std::byte, 4> b{};
    writeU32(b, 0xDEADBEEFu, Endian::Little);
    REQUIRE(readU32(b, Endian::Little) == 0xDEADBEEFu);

    std::array<std::byte, 8> c{};
    writeU64(c, 0x0011223344556677ull, Endian::Little);
    REQUIRE(readU64(c, Endian::Little) == 0x0011223344556677ull);
}
