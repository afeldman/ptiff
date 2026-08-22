#pragma once

#include <cstdint>
#include <unordered_map>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/tiff_endian.hpp>
#include <ptiff/io/binary_reader.hpp>

namespace ptiff::io::backend::tiff {

/// One parsed IFD: every entry's tag id mapped to its fully-resolved array of unsigned 64-bit
/// values. readTiffIfd() resolves both inline and offset-indirected entries eagerly, so callers
/// never see the raw on-disk encoding.
class TiffIfd {
public:
    void setTag(std::uint16_t tagId, std::vector<std::uint64_t> values);

    /// Error::NotFound if `tagId` was never set in this IFD.
    [[nodiscard]] Result<std::vector<std::uint64_t>> tag(std::uint16_t tagId) const;
    /// Convenience for single-value tags. Error::NotFound if absent, Error::InvalidArgument if
    /// the tag is present with zero values.
    [[nodiscard]] Result<std::uint64_t> singleValue(std::uint16_t tagId) const;
    /// `fallback` if `tagId` is absent; still Error::InvalidArgument if present with zero values.
    [[nodiscard]] Result<std::uint64_t> singleValueOr(std::uint16_t tagId,
                                                      std::uint64_t fallback) const;

    /// Absolute file offset of the following IFD in the chain, or 0 if this is the last IFD.
    /// This is the trailing next-IFD field (a fixed 4/8-byte trailer, not a tag entry) that
    /// readTiffIfd() reads after the entry table. 0 means "no further IFD" -- the single-IFD
    /// files this backend has always written end here.
    [[nodiscard]] std::uint64_t nextIfdOffset() const noexcept { return nextIfdOffset_; }
    /// Sets the trailing next-IFD field value; used by readTiffIfd().
    void setNextIfdOffset(std::uint64_t offset) noexcept { nextIfdOffset_ = offset; }

private:
    std::unordered_map<std::uint16_t, std::vector<std::uint64_t>> tags_;
    std::uint64_t nextIfdOffset_ = 0;
};

/// Parses the IFD at `ifdOffset` (classic 12-byte or BigTIFF 20-byte entries per `isBigTiff`),
/// resolving every entry's value eagerly via `reader`. Error::InvalidArgument on a truncated IFD,
/// an entry whose field type this backend does not understand, or a value/offset read failure.
[[nodiscard]] Result<TiffIfd>
readTiffIfd(BinaryReader& reader, std::uint64_t ifdOffset, Endian endian, bool isBigTiff);

} // namespace ptiff::io::backend::tiff
