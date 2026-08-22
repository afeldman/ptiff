#include <ptiff/compression/deflate.hpp>

// C API, confined to this translation unit -- never let zlib types reach the public header.
extern "C" {
#include <zlib.h>
}

namespace ptiff::compression {

Result<std::vector<std::byte>> decodeDeflate(std::span<const std::byte> input,
                                             std::size_t expectedSize) {
    std::vector<std::byte> output(expectedSize);
    uLongf outLen = static_cast<uLongf>(expectedSize);
    const int rc = uncompress(reinterpret_cast<Bytef*>(output.data()),
                              &outLen,
                              reinterpret_cast<const Bytef*>(input.data()),
                              static_cast<uLong>(input.size()));
    if (rc != Z_OK) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "decodeDeflate: zlib decompression failed"});
    }
    if (outLen != expectedSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "decodeDeflate: decoded output does not match expectedSize"});
    }
    return output;
}

Result<std::vector<std::byte>> encodeDeflate(std::span<const std::byte> input) {
    if (input.size() > 0xFFFFFFFFU) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "encodeDeflate: input exceeds uint32 strip byte count"});
    }
    std::vector<std::byte> output(compressBound(static_cast<uLong>(input.size())));
    uLongf outLen = static_cast<uLongf>(output.size());
    const int rc = compress2(reinterpret_cast<Bytef*>(output.data()),
                             &outLen,
                             reinterpret_cast<const Bytef*>(input.data()),
                             static_cast<uLong>(input.size()),
                             Z_DEFAULT_COMPRESSION);
    if (rc != Z_OK) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeDeflate: zlib compression failed"});
    }
    output.resize(outLen);
    return output;
}

} // namespace ptiff::compression
