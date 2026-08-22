#pragma once

#include <cstddef>
#include <cstdint>
#include <span>

#include <ptiff/core/precondition.hpp>

namespace ptiff::io::backend::tiff {

/// Byte order a TIFF/BigTIFF file declares in its header ("II" = little-endian, "MM" =
/// big-endian). Every multi-byte field in the file, including IFD entries, is encoded in this
/// order.
enum class Endian : std::uint8_t { Little, Big };

/// Reads a 2-byte unsigned integer from the first 2 bytes of `bytes`, honoring `endian`.
[[nodiscard]] constexpr std::uint16_t readU16(std::span<const std::byte> bytes,
                                              Endian endian) noexcept {
    PTIFF_PRECONDITION(bytes.size() >= 2);
    const auto b0 = std::to_integer<std::uint16_t>(bytes[0]);
    const auto b1 = std::to_integer<std::uint16_t>(bytes[1]);
    return endian == Endian::Little ? static_cast<std::uint16_t>(b0 | (b1 << 8))
                                    : static_cast<std::uint16_t>((b0 << 8) | b1);
}

/// Reads a 4-byte unsigned integer from the first 4 bytes of `bytes`, honoring `endian`.
[[nodiscard]] constexpr std::uint32_t readU32(std::span<const std::byte> bytes,
                                              Endian endian) noexcept {
    PTIFF_PRECONDITION(bytes.size() >= 4);
    const auto b0 = std::to_integer<std::uint32_t>(bytes[0]);
    const auto b1 = std::to_integer<std::uint32_t>(bytes[1]);
    const auto b2 = std::to_integer<std::uint32_t>(bytes[2]);
    const auto b3 = std::to_integer<std::uint32_t>(bytes[3]);
    return endian == Endian::Little ? (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
                                    : ((b0 << 24) | (b1 << 16) | (b2 << 8) | b3);
}

/// Reads an 8-byte unsigned integer from the first 8 bytes of `bytes`, honoring `endian`.
[[nodiscard]] constexpr std::uint64_t readU64(std::span<const std::byte> bytes,
                                              Endian endian) noexcept {
    PTIFF_PRECONDITION(bytes.size() >= 8);
    std::uint64_t value = 0;
    for (std::size_t i = 0; i < 8; ++i) {
        const auto byte = std::to_integer<std::uint64_t>(bytes[i]);
        const auto shift = static_cast<unsigned>(endian == Endian::Little ? i : 7 - i) * 8U;
        value |= byte << shift;
    }
    return value;
}

/// Writes `value` into the first 2 bytes of `out`, honoring `endian`.
constexpr void writeU16(std::span<std::byte> out, std::uint16_t value, Endian endian) noexcept {
    PTIFF_PRECONDITION(out.size() >= 2);
    const auto b0 = static_cast<std::byte>(value & 0xFF);
    const auto b1 = static_cast<std::byte>((value >> 8) & 0xFF);
    if (endian == Endian::Little) {
        out[0] = b0;
        out[1] = b1;
    } else {
        out[0] = b1;
        out[1] = b0;
    }
}

/// Writes `value` into the first 4 bytes of `out`, honoring `endian`.
constexpr void writeU32(std::span<std::byte> out, std::uint32_t value, Endian endian) noexcept {
    PTIFF_PRECONDITION(out.size() >= 4);
    for (std::size_t i = 0; i < 4; ++i) {
        const auto shift = static_cast<unsigned>(endian == Endian::Little ? i : 3 - i) * 8U;
        out[i] = static_cast<std::byte>((value >> shift) & 0xFF);
    }
}

/// Writes `value` into the first 8 bytes of `out`, honoring `endian`.
constexpr void writeU64(std::span<std::byte> out, std::uint64_t value, Endian endian) noexcept {
    PTIFF_PRECONDITION(out.size() >= 8);
    for (std::size_t i = 0; i < 8; ++i) {
        const auto shift = static_cast<unsigned>(endian == Endian::Little ? i : 7 - i) * 8U;
        out[i] = static_cast<std::byte>((value >> shift) & 0xFF);
    }
}

} // namespace ptiff::io::backend::tiff
