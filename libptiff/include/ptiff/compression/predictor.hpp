#pragma once

#include <cstddef>
#include <cstdint>
#include <span>

#include <ptiff/core/result.hpp>

namespace ptiff::compression {

/// Undoes TIFF Predictor=2 (horizontal differencing) in place. Each row of `data` was encoded,
/// independently per interleaved sample component (component c of column x was differenced
/// against component c of column x-1; differencing never crosses a row boundary), as a running
/// sum modulo 2^(8*bytesPerSample) -- this reverses it with a running sum, reset at the start of
/// every row. `bytesPerSample` must be 1, 2, or 4 (this backend's only supported BitsPerSample
/// values); multi-byte samples are read/written honoring `bigEndian`, matching the TIFF file's
/// declared byte order (the same order the samples were stored in before compression).
/// `data.size()` must be an exact multiple of `rowWidth * samplesPerPixel * bytesPerSample` --
/// any other size, or an unsupported `bytesPerSample`, is Error::InvalidArgument.
[[nodiscard]] Result<void> undoHorizontalDifferencing(std::span<std::byte> data,
                                                      std::uint32_t rowWidth,
                                                      std::uint32_t samplesPerPixel,
                                                      std::uint8_t bytesPerSample,
                                                      bool bigEndian);

/// Applies TIFF Predictor=2 (horizontal differencing) in place, the exact inverse of
/// undoHorizontalDifferencing. Each row's samples are differenced per interleaved component
/// (component c of column x becomes itself minus component c of column x-1), computed as a
/// running subtraction modulo 2^(8*bytesPerSample), reset at the start of every row. bytesPerSample
/// must be 1, 2, or 4; data.size() must be a multiple of rowWidth * samplesPerPixel *
/// bytesPerSample, else Error::InvalidArgument.
[[nodiscard]] Result<void> applyHorizontalDifferencing(std::span<std::byte> data,
                                                       std::uint32_t rowWidth,
                                                       std::uint32_t samplesPerPixel,
                                                       std::uint8_t bytesPerSample,
                                                       bool bigEndian);

} // namespace ptiff::compression
