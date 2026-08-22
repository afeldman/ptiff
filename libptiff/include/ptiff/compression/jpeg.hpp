#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>

namespace ptiff::compression {

/// Encodes `pixels` (row-major, `width * height * samplesPerPixel` bytes, one byte per sample --
/// baseline JPEG is 8-bit-only) as a JPEG stream (TIFF Compression=7, "new-style JPEG").
/// `samplesPerPixel` must be 1 (grayscale, JCS_GRAYSCALE) or 3 (RGB, JCS_RGB -- libjpeg-turbo
/// performs the RGB->YCbCr transform internally, matching TIFF Technical Note 2's YCbCr
/// convention). No chroma subsampling: every component's sampling factor is forced to 1x1
/// (4:4:4). `quality` is the libjpeg-turbo quality parameter, [0, 100].
/// Error::InvalidArgument if `quality` is out of range, `samplesPerPixel` is neither 1 nor 3,
/// `pixels.size() != width * height * samplesPerPixel`, or the underlying libjpeg-turbo encoder
/// reports failure.
[[nodiscard]] Result<std::vector<std::byte>> encodeJpeg(std::span<const std::byte> pixels,
                                                        std::uint32_t width,
                                                        std::uint32_t height,
                                                        std::uint32_t samplesPerPixel,
                                                        int quality);

/// Decodes a JPEG stream produced by encodeJpeg (or any compliant new-style-JPEG TIFF strip) back
/// to `width * height * samplesPerPixel` raw bytes, row-major, one byte per sample. Chroma
/// subsampling in the incoming stream is handled transparently by libjpeg-turbo (it reads the
/// actual subsampling from the stream's own SOF markers, not from any TIFF tag) -- the codec
/// boundary is always RGB/grayscale bytes in, RGB/grayscale bytes out.
/// Error::InvalidArgument on a malformed/truncated JPEG stream, or if the decoded image's
/// width/height/component count does not exactly match width/height/samplesPerPixel.
[[nodiscard]] Result<std::vector<std::byte>> decodeJpeg(std::span<const std::byte> input,
                                                        std::uint32_t width,
                                                        std::uint32_t height,
                                                        std::uint32_t samplesPerPixel);

} // namespace ptiff::compression
