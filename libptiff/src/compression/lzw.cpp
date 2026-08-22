#include <cstdint>
#include <map>
#include <utility>

#include <ptiff/compression/lzw.hpp>

namespace ptiff::compression {

namespace {

constexpr std::uint16_t clearCode = 256;
constexpr std::uint16_t eoiCode = 257;
constexpr std::uint16_t firstDictionaryCode = 258;
constexpr std::uint16_t maxDictionaryCode = 4094;

/// TIFF's "early change" code-width schedule: width grows one code sooner than a naive scheme
/// (at 511/1023/2047 instead of 512/1024/2048).
constexpr std::uint8_t codeWidthFor(std::uint16_t nextCode) noexcept {
    if (nextCode >= 2047) {
        return 12;
    }
    if (nextCode >= 1023) {
        return 11;
    }
    if (nextCode >= 511) {
        return 10;
    }
    return 9;
}

using Table = std::map<std::pair<std::vector<std::byte>, std::byte>, std::uint16_t>;

// Returns the LZW code for a phrase: its own byte for a 1-byte phrase (codes 0..255, which the
// decoder seeds implicitly), or the table code assigned by the encoder for longer phrases
// (258..4094). Seq is always a phrase the encoder previously inserted in its `Table`, so the
// table's `->second` gives the code. The single-byte case maps to the byte value directly
// (codes 0..255); nothing else in this encoder emits a code below 258.
std::uint16_t codeForByteSequence(const std::vector<std::byte>& seq, const Table& table) {
    if (seq.size() == 1) {
        return static_cast<std::uint8_t>(std::to_integer<std::uint8_t>(seq[0]));
    }
    // Longer phrases: rebuild the (prefix, lastByte) key that insert stored, and fetch its code.
    const std::vector<std::byte> prefix{seq.begin(), seq.end() - 1};
    return table.at(std::make_pair(prefix, seq.back()));
}

// Packs `codes` MSB-first, honoring the decoder's width transitions. The decoder reads each
// code with its current `codeWidth`, and only grows the width from the 4th dictionary entry
// AFTER a non-clear/non-EOI code (via codeWidthFor(nextCode)), AND only when it already holds an
// old entry (`haveOldEntry`) — i.e. never for the first normal code after a clear. So the width
// used for output code i is computed by replaying the same state machine: start after a clear at
// 9 bits; a normal code (not clear/EOI) at table-size S uses `codeWidthFor(S)`, and only if we
// already emitted a previous normal code since the last clear does it then advance the table size
// by 1. This mirrors the decoder's `haveOldEntry` gating so the width schedule stays in sync at
// the early-change boundaries (511/1023/2047). This function re-derives that width per index in
// one forward pass over the code list.
Result<std::vector<std::byte>> writeCodesToBits(const std::vector<std::uint32_t>& codes) {
    std::vector<std::byte> out;
    std::uint32_t bitsAcc = 0; // pending bits, MSB-aligned in the accumulator
    unsigned bitsPending = 0;
    std::uint16_t tableSize = firstDictionaryCode; // 258 after a fresh clear
    bool havePrevious = false;                     // mirrors the decoder's haveOldEntry gating
    auto width = [&]() { return codeWidthFor(tableSize); };

    auto flushByte = [&]() {
        out.push_back(static_cast<std::byte>((bitsAcc >> (bitsPending - 8)) & 0xFFU));
        bitsPending -= 8;
    };

    for (const auto code : codes) {
        const std::uint8_t w = width(); // decoder's width when reading this code
        bitsAcc = (bitsAcc << w) | (code & ((1U << w) - 1U));
        bitsPending += w;
        while (bitsPending >= 8)
            flushByte();

        if (code == clearCode) {
            tableSize = firstDictionaryCode; // decoder resets to 9 bits on a clear
            havePrevious = false;            // ...and forgets its old entry too
        } else if (code != eoiCode) {
            // Only grow the table once we already hold a previous normal code, exactly as the
            // decoder grows nextCode only when haveOldEntry is true (never for the first normal
            // code after a clear).
            if (havePrevious && tableSize < maxDictionaryCode) {
                ++tableSize;
            }
            havePrevious = true;
        }
    }
    // Emit any trailing zero-padded partial byte so the stream is byte-aligned (the decoder
    // stops at EOI, ignoring trailing padding).
    if (bitsPending > 0) {
        out.push_back(static_cast<std::byte>((bitsAcc << (8 - bitsPending)) & 0xFFU));
    }
    return out;
}

} // namespace

Result<std::vector<std::byte>> decodeLzw(std::span<const std::byte> input,
                                         std::size_t expectedSize) {
    std::vector<std::vector<std::byte>> table(firstDictionaryCode);
    for (int i = 0; i < 256; ++i) {
        table[static_cast<std::size_t>(i)] = {static_cast<std::byte>(i)};
    }

    std::size_t bitPos = 0;
    const std::size_t totalBits = input.size() * 8;

    auto readCode = [&](std::uint8_t width) -> Result<std::uint16_t> {
        if (bitPos + width > totalBits) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "decodeLzw: bitstream exhausted before EOI"});
        }
        std::uint16_t code = 0;
        for (std::uint8_t i = 0; i < width; ++i) {
            const std::size_t byteIndex = bitPos / 8;
            const std::size_t bitIndex = 7 - (bitPos % 8);
            const auto bit = (std::to_integer<std::uint8_t>(input[byteIndex]) >> bitIndex) & 1U;
            code = static_cast<std::uint16_t>((code << 1) | static_cast<std::uint16_t>(bit));
            ++bitPos;
        }
        return code;
    };

    std::vector<std::byte> output;
    output.reserve(expectedSize);
    std::uint16_t nextCode = firstDictionaryCode;
    std::uint8_t codeWidth = 9;
    bool haveOldEntry = false;
    std::vector<std::byte> oldEntry;

    auto appendBounded = [&](const std::vector<std::byte>& entry) -> Result<void> {
        if (output.size() + entry.size() > expectedSize) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "decodeLzw: decoded output exceeds expectedSize"});
        }
        output.insert(output.end(), entry.begin(), entry.end());
        return {};
    };

    while (true) {
        auto code = readCode(codeWidth);
        if (!code.has_value()) {
            return std::unexpected(code.error());
        }

        if (*code == clearCode) {
            table.resize(firstDictionaryCode);
            nextCode = firstDictionaryCode;
            codeWidth = 9;
            haveOldEntry = false;
            continue;
        }
        if (*code == eoiCode) {
            break;
        }

        std::vector<std::byte> entry;
        if (*code < 256) {
            entry = table[*code];
        } else if (*code >= firstDictionaryCode && *code < nextCode) {
            entry = table[*code];
        } else if (*code == nextCode && haveOldEntry) {
            entry = oldEntry;
            entry.push_back(oldEntry.front());
        } else {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "decodeLzw: invalid code in compressed stream"});
        }

        auto appended = appendBounded(entry);
        if (!appended.has_value()) {
            return std::unexpected(appended.error());
        }

        if (haveOldEntry) {
            if (nextCode > maxDictionaryCode) {
                return std::unexpected(Error{ErrorCode::InvalidArgument,
                                             "decodeLzw: dictionary exceeded 12-bit table"});
            }
            std::vector<std::byte> newEntry = oldEntry;
            newEntry.push_back(entry.front());
            if (static_cast<std::size_t>(nextCode) >= table.size()) {
                table.resize(static_cast<std::size_t>(nextCode) + 1);
            }
            table[nextCode] = std::move(newEntry);
            ++nextCode;
            codeWidth = codeWidthFor(nextCode);
        }

        oldEntry = entry;
        haveOldEntry = true;
    }

    if (output.size() != expectedSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "decodeLzw: decoded output shorter than expectedSize"});
    }
    return output;
}

Result<std::vector<std::byte>> encodeLzw(std::span<const std::byte> input) {
    if (input.size() > 0xFFFFFFFFU) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeLzw: input exceeds uint32 strip byte count"});
    }

    std::vector<std::uint32_t> codes;
    codes.push_back(clearCode); // start with a clear so decoders reset their table

    // The first 256 single-byte entries are implicit in the decoder (table[0..255] = the byte).
    // Store phrases (dictionaryStart, lookupByte) -> code. A naive map over growing byte vectors is
    // correct but O(n^2); an acceptable simplification here given the small strips this backend
    // writes. The existing decoder only requires the emitted codes to be valid; correctness over
    // speed is the goal.
    Table table;

    std::size_t pos = 0;
    std::uint16_t nextCode = firstDictionaryCode; // 258
    std::uint8_t codeWidth = 9;
    auto resetToFull = [&]() {
        nextCode = firstDictionaryCode;
        codeWidth = 9;
        table.clear();
    };

    std::vector<std::byte> current;
    if (pos < input.size())
        current.push_back(input[pos++]);

    while (pos < input.size()) {
        const std::byte next = input[pos];
        std::pair<std::vector<std::byte>, std::byte> key{current, next};
        const auto it = table.find(key);
        if (it != table.end()) {
            current.push_back(next); // extend the current phrase
            ++pos;
        } else {
            // Emit the code for `current` at the current width.
            codes.push_back(codeForByteSequence(current, table));
            // Add the newly discovered phrase (current + next) to the table.
            if (nextCode < maxDictionaryCode) {
                table.emplace(std::move(key), nextCode);
                ++nextCode;
                if (nextCode > 2046)
                    codeWidth = 12;
                else if (nextCode > 1022)
                    codeWidth = 11;
                else if (nextCode > 510)
                    codeWidth = 10;
                if (nextCode >= maxDictionaryCode) {
                    // table is full: emit clear, reset, then start a fresh current phrase
                    codes.push_back(clearCode);
                    resetToFull();
                }
            }
            current.assign(1, next);
            ++pos;
        }
    }
    if (!current.empty()) {
        codes.push_back(codeForByteSequence(current, table));
    }
    codes.push_back(eoiCode);

    // Convert the code list to an MSB-first bitstream, growing width per the early-change table,
    // computed from the position in the code list the same way decodeLzw derives it (width used for
    // a code is codeWidthFor(nextCodeBeforeAddingThatCode)); zero-pad the final partial byte.
    return writeCodesToBits(codes);
}

} // namespace ptiff::compression
