#pragma once

namespace ptiff {

/// @brief The scalar type of one channel sample.
///
/// Identifies the numeric storage type of each pixel sample in an
/// @ref ptiff::ImageDescriptor "ImageDescriptor". Multi-channel images share a single
/// `PixelType` across all channels.
///
/// @section pixel_type_values Values
///
/// | Enumerator | Meaning |
/// |------------|---------|
/// | `UInt8`    | Unsigned 8-bit integer sample. |
/// | `UInt16`   | Unsigned 16-bit integer sample. |
/// | `UInt32`   | Unsigned 32-bit integer sample. |
/// | `Float32`  | IEEE-754 32-bit floating-point sample. |
/// | `Float64`  | IEEE-754 64-bit floating-point sample. |
///
/// @note This list is **additive** once released: existing enumerators are never renamed,
///       renumbered or removed, so downstream code may switch on them across versions.
enum class PixelType {
    UInt8,
    UInt16,
    UInt32,
    Float32,
    Float64,
};

} // namespace ptiff
