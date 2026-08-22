#include <cstdint>

#include <ptiff/compression/packbits.hpp>

namespace ptiff::compression {

Result<std::vector<std::byte>> decodePackBits(std::span<const std::byte> input,
                                              std::size_t expectedSize) {
    std::vector<std::byte> output;
    output.reserve(expectedSize);
    std::size_t pos = 0;

    while (output.size() < expectedSize) {
        if (pos >= input.size()) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "decodePackBits: input exhausted before expectedSize"});
        }
        const auto control = static_cast<std::int8_t>(std::to_integer<std::uint8_t>(input[pos]));
        ++pos;

        if (control >= 0) {
            const std::size_t count = static_cast<std::size_t>(control) + 1;
            if (pos + count > input.size()) {
                return std::unexpected(Error{ErrorCode::InvalidArgument,
                                             "decodePackBits: literal run reads past input end"});
            }
            if (output.size() + count > expectedSize) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument,
                          "decodePackBits: decoded output exceeds expectedSize"});
            }
            output.insert(output.end(),
                          input.begin() + static_cast<std::ptrdiff_t>(pos),
                          input.begin() + static_cast<std::ptrdiff_t>(pos + count));
            pos += count;
        } else if (control != -128) {
            const std::size_t count = static_cast<std::size_t>(-control) + 1;
            if (pos >= input.size()) {
                return std::unexpected(Error{ErrorCode::InvalidArgument,
                                             "decodePackBits: repeat run missing its byte"});
            }
            if (output.size() + count > expectedSize) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument,
                          "decodePackBits: decoded output exceeds expectedSize"});
            }
            const std::byte value = input[pos];
            ++pos;
            output.insert(output.end(), count, value);
        }
        // control == -128: no-op, consumes only the control byte, produces nothing.
    }

    if (output.size() != expectedSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "decodePackBits: decoded output does not match expectedSize"});
    }

    return output;
}

Result<std::vector<std::byte>> encodePackBits(std::span<const std::byte> input) {
    if (input.size() > 0xFFFFFFFFU) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "encodePackBits: input exceeds uint32 strip byte count"});
    }
    std::vector<std::byte> out;
    out.reserve(input.size());
    const std::size_t n = input.size();
    std::size_t pos = 0;
    while (pos < n) {
        // A run of >= 2 identical bytes is encoded as a repeat control (legal and decodes fine).
        std::size_t runLen = 1;
        while (runLen < n - pos && input[pos + runLen] == input[pos])
            ++runLen;
        if (runLen >= 2) {
            const std::size_t runLenBefore = runLen;
            while (runLen > 0) {
                const std::size_t chunk = runLen < 128 ? runLen : 128;
                // control byte = -(chunk - 1): for chunk 2 -> -1, chunk 3 -> -2, ... chunk 128 ->
                // -127
                out.push_back(static_cast<std::byte>(
                    static_cast<std::int8_t>(-static_cast<std::int16_t>(chunk - 1))));
                out.push_back(input[pos]);
                runLen -= chunk;
            }
            pos += runLenBefore;
            continue;
        }
        // Literal run: gather up to 128 bytes, stopping just before a run of >= 2 begins.
        const std::size_t litStart = pos;
        ++pos;
        while (pos < n && (pos - litStart) < 128 && input[pos] != input[pos - 1]) {
            ++pos;
        }
        const std::size_t literalCount = pos - litStart;
        out.push_back(static_cast<std::byte>(literalCount - 1));
        out.insert(out.end(),
                   input.begin() + static_cast<std::ptrdiff_t>(litStart),
                   input.begin() + static_cast<std::ptrdiff_t>(pos));
    }
    return out;
}

} // namespace ptiff::compression
