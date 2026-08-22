#pragma once

#include <cstdint>

#include <ptiff/io/backend/tiff/policy/tiff_policy_fwd.hpp>

namespace ptiff::io::backend::tiff::policy {

/// A PartitionPolicy decides how an image is divided into tiles and which layout tags the writer
/// emits (strip: RowsPerStrip/StripOffsets/StripByteCounts; tile:
/// TileWidth/TileLength/TileOffsets/TileByteCounts). It provides constexpr geometry helpers so
/// offsets/byte counts can be computed at compile time; the concrete per-tile byte range values
/// still depend on runtime image dimensions and land in the runtime IFD.
template <typename T>
concept PartitionPolicy = requires {
    { T::isTiled } -> std::convertible_to<bool>;
    { T::tileWidth } -> std::convertible_to<std::uint32_t>;
    { T::tileHeight } -> std::convertible_to<std::uint32_t>;
};

/// Strip partition: the whole image is one (RowsPerStrip = imageHeight) strip covering the full
/// width -- this backend's baseline layout.
struct StripPolicy {
    static constexpr bool isTiled = false;
    static constexpr std::uint32_t tileWidth = 0;  // unused for strips
    static constexpr std::uint32_t tileHeight = 0; // unused for strips
};

/// Tiled partition with a fixed (compile-time) tile extent. `tileWidth`/`tileHeight` must be
/// nonzero multiples of 16 (TIFF 6.0 baseline requirement for tiled images). Off-spec values are
/// rejected by the writer with a static_assert, so bad tile geometry never compiles.
template <std::uint32_t W, std::uint32_t H> struct TiledPolicy {
    static_assert(W >= 16 && H >= 16 && W % 16 == 0 && H % 16 == 0,
                  "TiledPolicy: tile width/height must be nonzero multiples of 16");

    static constexpr bool isTiled = true;
    static constexpr std::uint32_t tileWidth = W;
    static constexpr std::uint32_t tileHeight = H;

    /// Number of tile columns covering an image of `imageWidth` pixels.
    [[nodiscard]] static constexpr std::uint32_t columns(std::uint32_t imageWidth) noexcept {
        return (imageWidth + W - 1) / W;
    }

    /// Number of tile rows covering an image of `imageHeight` pixels.
    [[nodiscard]] static constexpr std::uint32_t rows(std::uint32_t imageHeight) noexcept {
        return (imageHeight + H - 1) / H;
    }
};

} // namespace ptiff::io::backend::tiff::policy
