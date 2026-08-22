#pragma once

#include <cstdint>

#include <ptiff/io/backend/tiff/policy/tiff_policy_fwd.hpp>

namespace ptiff::io::backend::tiff::policy {

/// A PixelPolicy describes a single error-free, uniform sample format for a whole image: the
/// TIFF BitsPerSample, SampleFormat, and the byte size of one sample. Every member is constexpr
/// so policy-provided dimension math can run at compile time.
///
/// A conforming PixelPolicy must expose:
///   static constexpr PixelValue value;
///   static constexpr std::uint16_t bitsPerSample;
///   static constexpr std::uint16_t sampleFormat; // 1 = unsigned int, 3 = IEEE float
///   static constexpr std::uint16_t bytesPerSample; // bits / 8
template <typename T>
concept PixelPolicy = requires {
    { T::value } -> std::convertible_to<PixelValue>;
    { T::bitsPerSample } -> std::convertible_to<std::uint16_t>;
    { T::sampleFormat } -> std::convertible_to<std::uint16_t>;
    { T::bytesPerSample } -> std::convertible_to<std::uint16_t>;
};

/// 8-bit unsigned grayscale sample.
struct UInt8Policy {
    static constexpr PixelValue value = PixelValue::UInt8;
    static constexpr std::uint16_t bitsPerSample = 8;
    static constexpr std::uint16_t sampleFormat = 1; // unsigned int
    static constexpr std::uint16_t bytesPerSample = 1;
};

/// 16-bit unsigned sample.
struct UInt16Policy {
    static constexpr PixelValue value = PixelValue::UInt16;
    static constexpr std::uint16_t bitsPerSample = 16;
    static constexpr std::uint16_t sampleFormat = 1;
    static constexpr std::uint16_t bytesPerSample = 2;
};

/// 32-bit unsigned sample.
struct UInt32Policy {
    static constexpr PixelValue value = PixelValue::UInt32;
    static constexpr std::uint16_t bitsPerSample = 32;
    static constexpr std::uint16_t sampleFormat = 1;
    static constexpr std::uint16_t bytesPerSample = 4;
};

/// IEEE 754 32-bit float sample.
struct Float32Policy {
    static constexpr PixelValue value = PixelValue::Float32;
    static constexpr std::uint16_t bitsPerSample = 32;
    static constexpr std::uint16_t sampleFormat = 3; // IEEE float
    static constexpr std::uint16_t bytesPerSample = 4;
};

} // namespace ptiff::io::backend::tiff::policy
