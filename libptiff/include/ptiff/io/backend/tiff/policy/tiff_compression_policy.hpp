#pragma once

#include <cstdint>

#include <ptiff/io/backend/tiff/policy/tiff_policy_fwd.hpp>

namespace ptiff::io::backend::tiff::policy {

/// A CompressionPolicy chooses the TIFF Compression tag value (259) and reports whether the
/// policy writer can currently encode it end-to-end. The write path currently ships full
/// uncompressed writes; PackBits/LZW expose the correct tag value and the (existing) encoder
/// entry points, but their "encode a strip/tile byte-for-byte" path is a documented
/// not-yet-shipped write feature -- exactly the same cut the runtime planTiffWrite makes
/// (tiled + compression is rejected there too).
template <typename T>
concept CompressionPolicy = requires {
    { T::kTagValue } -> std::convertible_to<std::uint16_t>;
    { T::kEncodeSupported } -> std::convertible_to<bool>;
};

/// Uncompressed (Compression = 1). The full supported write path: tile/strip byte counts are
/// known exactly up front, no back-patching needed.
struct NonePolicy {
    static constexpr std::uint16_t kTagValue = 1;
    static constexpr bool kEncodeSupported = true;
};

/// TIFF PackBits (Compression = 32773). Tag + encoder entry points are present
/// (ptiff::compression::encodePackBits); the full compressed write path is not yet shipped.
struct PackBitsPolicy {
    static constexpr std::uint16_t kTagValue = 32773;
    static constexpr bool kEncodeSupported = false;
};

/// TIFF LZW (Compression = 5). Tag + encoder entry points are present
/// (ptiff::compression::encodeLzw); the full compressed write path is not yet shipped.
struct LzwPolicy {
    static constexpr std::uint16_t kTagValue = 5;
    static constexpr bool kEncodeSupported = false;
};

} // namespace ptiff::io::backend::tiff::policy
