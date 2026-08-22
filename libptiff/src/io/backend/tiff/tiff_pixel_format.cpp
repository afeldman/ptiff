#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>

namespace ptiff::io::backend::tiff {

Result<ptiff::PixelType> resolvePixelType(std::uint64_t bitsPerSample, std::uint64_t sampleFormat) {
    if (sampleFormat != 1 && sampleFormat != 3) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "resolvePixelType: unsupported SampleFormat"});
    }
    if (sampleFormat == 3) {
        if (bitsPerSample != 32) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "resolvePixelType: float SampleFormat requires "
                                         "32-bit BitsPerSample"});
        }
        return ptiff::PixelType::Float32;
    }
    switch (bitsPerSample) {
    case 8: {
        return ptiff::PixelType::UInt8;
    }
    case 16: {
        return ptiff::PixelType::UInt16;
    }
    case 32: {
        return ptiff::PixelType::UInt32;
    }
    default: {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "resolvePixelType: unsupported BitsPerSample"});
    }
    }
}

Result<void> requireUniformBitsPerSample(const std::vector<std::uint64_t>& values) {
    if (values.empty()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "requireUniformBitsPerSample: no values"});
    }
    for (auto value : values) {
        if (value != values.front()) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "requireUniformBitsPerSample: non-uniform BitsPerSample"});
        }
    }
    return {};
}

std::string_view pixelTypeFieldValue(ptiff::PixelType pixelType) noexcept {
    switch (pixelType) {
    case ptiff::PixelType::UInt8: {
        return "UInt8";
    }
    case ptiff::PixelType::UInt16: {
        return "UInt16";
    }
    case ptiff::PixelType::UInt32: {
        return "UInt32";
    }
    case ptiff::PixelType::Float32: {
        return "Float32";
    }
    case ptiff::PixelType::Float64: {
        return "Float64";
    }
    }
    return "UInt8";
}

std::uint8_t bytesPerSample(ptiff::PixelType pixelType) noexcept {
    switch (pixelType) {
    case ptiff::PixelType::UInt8: {
        return 1;
    }
    case ptiff::PixelType::UInt16: {
        return 2;
    }
    case ptiff::PixelType::UInt32:
    case ptiff::PixelType::Float32: {
        return 4;
    }
    case ptiff::PixelType::Float64: {
        return 8;
    }
    }
    return 1;
}

Result<ptiff::PixelType> pixelTypeFromFieldValue(std::string_view value) {
    if (value == "UInt8") {
        return ptiff::PixelType::UInt8;
    }
    if (value == "UInt16") {
        return ptiff::PixelType::UInt16;
    }
    if (value == "UInt32") {
        return ptiff::PixelType::UInt32;
    }
    if (value == "Float32") {
        return ptiff::PixelType::Float32;
    }
    if (value == "Float64") {
        return ptiff::PixelType::Float64;
    }
    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                 "pixelTypeFromFieldValue: unrecognized pixelType field value"});
}

std::uint16_t bitsPerSampleFor(ptiff::PixelType pixelType) noexcept {
    return static_cast<std::uint16_t>(bytesPerSample(pixelType)) * 8U;
}

std::uint16_t sampleFormatFor(ptiff::PixelType pixelType) noexcept {
    return pixelType == ptiff::PixelType::Float32 || pixelType == ptiff::PixelType::Float64 ? 3U
                                                                                            : 1U;
}

} // namespace ptiff::io::backend::tiff
