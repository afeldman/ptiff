#pragma once

#include <cstdint>
#include <string_view>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/image/pixel_type.hpp>

namespace ptiff::io::backend::tiff {

/// Maps TIFF's (BitsPerSample, SampleFormat) pair to ptiff::PixelType, for this backend's
/// supported subset: unsigned int at 8/16/32 bits, or IEEE float at 32 bits only.
/// SampleFormat: 1 = unsigned int (TIFF's default when the tag is absent), 3 = IEEE float. Any
/// other SampleFormat, or a BitsPerSample unsupported for the given format, is
/// Error::InvalidArgument.
[[nodiscard]] Result<ptiff::PixelType> resolvePixelType(std::uint64_t bitsPerSample,
                                                        std::uint64_t sampleFormat);

/// Error::InvalidArgument if `values` is empty or contains more than one distinct value -- this
/// backend requires BitsPerSample to be uniform across all samples of a pixel.
[[nodiscard]] Result<void> requireUniformBitsPerSample(const std::vector<std::uint64_t>& values);

/// The StorageModel field-value string for `pixelType` (e.g. "UInt8", "Float32") -- the inverse
/// of the BitsPerSample/SampleFormat mapping, used by tiff_directory's toStorageModel.
[[nodiscard]] std::string_view pixelTypeFieldValue(ptiff::PixelType pixelType) noexcept;

/// Byte size of one sample of `pixelType` (1/2/4 for UInt8/UInt16/(UInt32,Float32)). Shared by
/// the read path (TiffImageSource, expected-tile-size computation) and the write path
/// (tiff_directory_writer, StripByteCounts computation).
[[nodiscard]] std::uint8_t bytesPerSample(ptiff::PixelType pixelType) noexcept;

/// Inverse of pixelTypeFieldValue -- parses a StorageModel "pixelType" field string back into a
/// ptiff::PixelType. Error::InvalidArgument if `value` isn't one of the strings
/// pixelTypeFieldValue produces.
[[nodiscard]] Result<ptiff::PixelType> pixelTypeFromFieldValue(std::string_view value);

/// The TIFF BitsPerSample value to write for `pixelType` -- the inverse (together with
/// sampleFormatFor) of resolvePixelType.
[[nodiscard]] std::uint16_t bitsPerSampleFor(ptiff::PixelType pixelType) noexcept;

/// The TIFF SampleFormat value to write for `pixelType` (1 = unsigned int, 3 = IEEE float) --
/// the inverse (together with bitsPerSampleFor) of resolvePixelType.
[[nodiscard]] std::uint16_t sampleFormatFor(ptiff::PixelType pixelType) noexcept;

} // namespace ptiff::io::backend::tiff
