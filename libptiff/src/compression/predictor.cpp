#include <vector>

#include <ptiff/compression/predictor.hpp>

namespace ptiff::compression {

namespace {

std::uint32_t
readSample(std::span<const std::byte> bytes, std::uint8_t bytesPerSample, bool bigEndian) {
    std::uint32_t value = 0;
    for (std::uint8_t i = 0; i < bytesPerSample; ++i) {
        const auto byteValue = std::to_integer<std::uint32_t>(bytes[i]);
        const auto shift = static_cast<unsigned>(bigEndian ? bytesPerSample - 1 - i : i) * 8U;
        value |= byteValue << shift;
    }
    return value;
}

void writeSample(std::span<std::byte> bytes,
                 std::uint32_t value,
                 std::uint8_t bytesPerSample,
                 bool bigEndian) {
    for (std::uint8_t i = 0; i < bytesPerSample; ++i) {
        const auto shift = static_cast<unsigned>(bigEndian ? bytesPerSample - 1 - i : i) * 8U;
        bytes[i] = static_cast<std::byte>((value >> shift) & 0xFFU);
    }
}

} // namespace

Result<void> undoHorizontalDifferencing(std::span<std::byte> data,
                                        std::uint32_t rowWidth,
                                        std::uint32_t samplesPerPixel,
                                        std::uint8_t bytesPerSample,
                                        bool bigEndian) {
    if (bytesPerSample != 1 && bytesPerSample != 2 && bytesPerSample != 4) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "undoHorizontalDifferencing: unsupported bytesPerSample"});
    }
    const std::size_t rowStride =
        static_cast<std::size_t>(rowWidth) * samplesPerPixel * bytesPerSample;
    if (rowStride == 0 || data.size() % rowStride != 0) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "undoHorizontalDifferencing: data size is not a multiple of "
                                     "rowWidth * samplesPerPixel * bytesPerSample"});
    }

    const std::uint32_t mask =
        bytesPerSample == 4 ? 0xFFFFFFFFU : (1U << (bytesPerSample * 8U)) - 1U;
    const std::size_t rowCount = data.size() / rowStride;

    for (std::size_t row = 0; row < rowCount; ++row) {
        std::span<std::byte> rowBytes = data.subspan(row * rowStride, rowStride);
        std::vector<std::uint32_t> running(samplesPerPixel, 0);

        for (std::uint32_t col = 0; col < rowWidth; ++col) {
            for (std::uint32_t comp = 0; comp < samplesPerPixel; ++comp) {
                const std::size_t offset =
                    (static_cast<std::size_t>(col) * samplesPerPixel + comp) * bytesPerSample;
                std::span<std::byte> sampleBytes = rowBytes.subspan(offset, bytesPerSample);
                const std::uint32_t delta = readSample(sampleBytes, bytesPerSample, bigEndian);
                running[comp] = (running[comp] + delta) & mask;
                writeSample(sampleBytes, running[comp], bytesPerSample, bigEndian);
            }
        }
    }
    return {};
}

Result<void> applyHorizontalDifferencing(std::span<std::byte> data,
                                         std::uint32_t rowWidth,
                                         std::uint32_t samplesPerPixel,
                                         std::uint8_t bytesPerSample,
                                         bool bigEndian) {
    if (bytesPerSample != 1 && bytesPerSample != 2 && bytesPerSample != 4) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "applyHorizontalDifferencing: unsupported bytesPerSample"});
    }
    const std::size_t rowStride =
        static_cast<std::size_t>(rowWidth) * samplesPerPixel * bytesPerSample;
    if (rowStride == 0 || data.size() % rowStride != 0) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "applyHorizontalDifferencing: data size is not a multiple of "
                                     "rowWidth * samplesPerPixel * bytesPerSample"});
    }
    const std::uint32_t mask =
        bytesPerSample == 4 ? 0xFFFFFFFFU : (1U << (bytesPerSample * 8U)) - 1U;
    const std::size_t rowCount = data.size() / rowStride;

    for (std::size_t row = 0; row < rowCount; ++row) {
        std::vector<std::uint32_t> previous(samplesPerPixel, 0);
        for (std::uint32_t col = 0; col < rowWidth; ++col) {
            for (std::uint32_t comp = 0; comp < samplesPerPixel; ++comp) {
                const std::size_t offset =
                    (static_cast<std::size_t>(row) * rowStride) +
                    (static_cast<std::size_t>(col) * samplesPerPixel + comp) * bytesPerSample;
                const auto sample =
                    readSample(data.subspan(offset, bytesPerSample), bytesPerSample, bigEndian);
                writeSample(data.subspan(offset, bytesPerSample),
                            static_cast<std::uint32_t>((sample - previous[comp]) & mask),
                            bytesPerSample,
                            bigEndian);
                previous[comp] = sample;
            }
        }
    }
    return {};
}

} // namespace ptiff::compression
