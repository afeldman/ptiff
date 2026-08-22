#pragma once

#include <cstdint>

namespace ptiff::io::backend::tiff::policy {

/// A ContainerPolicy picks classic TIFF (8-byte header, 12-byte IFD records) vs BigTIFF (16-byte
/// header, 20-byte IFD records), mirroring the runtime planTiffWrite "container" field.
template <typename T>
concept ContainerPolicy = requires {
    { T::isBigTiff } -> std::convertible_to<bool>;
    { T::headerSize } -> std::convertible_to<std::uint64_t>;
};

/// Classic TIFF 6.0: 8-byte header, 12-byte IFD entry records.
struct ClassicContainer {
    static constexpr bool isBigTiff = false;
    static constexpr std::uint64_t headerSize = 8;
};

/// BigTIFF (TIFF extension): 16-byte header, 20-byte IFD entry records and 8-byte offsets.
struct BigTiffContainer {
    static constexpr bool isBigTiff = true;
    static constexpr std::uint64_t headerSize = 16;
};

} // namespace ptiff::io::backend::tiff::policy
