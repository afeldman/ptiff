#pragma once

#include <array>
#include <cstdint>
#include <tuple>
#include <type_traits>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_compression_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_container_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_partition_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_pixel_policy.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp> // writeTiffHeader (reused verbatim)
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp> // writeTiffIfd / tiffIfdByteSize (reused verbatim)
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io::backend::tiff::policy {

/// Samples-per-pixel the policy writer supports (1 = grayscale). Kept a constant so the writer
/// can branch (PhotometricInterpretation) with `if constexpr` rather than a runtime switch.
inline constexpr std::uint32_t kSamplesPerPixel = 1;

/// PhotometricInterpretation implied by a grayscale policy layout (1 = BlackIsZero). RGB (2) is
/// not part of this policy writer's supported subset yet.
inline constexpr std::uint32_t kPhotometricGrayscale = 1;

// ---------------------------------------------------------------------------
// Compile-time directory shape: which entries a policy combination selects, in TIFF-ascending tag
// order, with each entry's field type and compile-time-known value count. This is the "policy
// as compile-time table" piece -- consumers can static_assert/if-constexpr on it without touching
// runtime state.
// ---------------------------------------------------------------------------

/// One compile-time IFD entry descriptor. `Tag` and `FieldType` are TIFF tag-id / field-type
/// values; `N` is the number of on-disk elements (0 = runtime-dependent count).
template <std::uint16_t Tag, std::uint16_t FieldType, std::size_t N> struct StaticEntry {
    static constexpr std::uint16_t tag = Tag;
    static constexpr std::uint16_t fieldType = FieldType;
    static constexpr std::size_t count = N;
};

namespace detail {

/// Compile-time byte size of a field type, mirroring fieldTypeSize() in tiff_tag.hpp.
[[nodiscard]] constexpr std::uint8_t fieldTypeByteSize(std::uint16_t fieldType) noexcept {
    switch (fieldType) {
    case 1:
        return 1; // BYTE
    case 3:
        return 2; // SHORT
    case 4:
        return 4; // LONG
    case 16:
        return 8; // LONG8
    }
    return 0;
}

/// Compile-time entry-record byte size, mirroring entrySize() in tiff_ifd_writer.cpp.
template <ContainerPolicy Container>
[[nodiscard]] constexpr std::uint64_t entryRecordSize() noexcept {
    return Container::isBigTiff ? 20 : 12;
}

/// Compile-time "fixed part" of an IFD (entry count + records + next-IFD offset), given how many
/// entries are present. Out-of-line value bytes are added by the caller, mirroring
/// tiff_ifd_writer.cpp's tiffIfdByteSize arithmetic.
template <ContainerPolicy Container, std::size_t EntryCount>
[[nodiscard]] constexpr std::uint64_t fixedIfdSize() noexcept {
    const std::uint64_t countField = Container::isBigTiff ? 8 : 2;
    const std::uint64_t nextIfd = Container::isBigTiff ? 8 : 4;
    return countField + EntryCount * entryRecordSize<Container>() + nextIfd;
}

} // namespace detail

/// The compile-time IFD a policy combination selects. `EntryList` is a std::tuple of StaticEntry
/// in ascending tag order; `isTiled` distinguishes strip (RowsPerStrip/StripOffsets/
/// StripByteCounts) from tiled (TileWidth/TileLength/TileOffsets/TileByteCounts) directories.
template <typename Pixel,
          PartitionPolicy Partition,
          typename Compression,
          ContainerPolicy Container>
struct PolicyDirectory {
    static_assert(policy::PixelPolicy<Pixel>, "PolicyDirectory: Pixel is not a PixelPolicy");
    static_assert(policy::CompressionPolicy<Compression>,
                  "PolicyDirectory: Compression is not a CompressionPolicy");

    using TiledEntries = std::tuple<StaticEntry<322, 4, 1>, /* TileWidth       */
                                    StaticEntry<323, 4, 1>, /* TileLength      */
                                    StaticEntry<324, 4, 0>, /* TileOffsets     (runtime count) */
                                    StaticEntry<325, 4, 0> /* TileByteCounts  (runtime count) */>;
    static constexpr bool isTiled = Partition::isTiled;
    static constexpr std::uint32_t photometric = kPhotometricGrayscale;

    /// Total entry count this directory selects (compile-time; runtime-value-counts still fill at
    /// materialization). Strip = 10 entries (7 common + RowsPerStrip/StripOffsets/StripByteCounts);
    /// tiled = 11 entries (7 common + TileWidth/TileLength/TileOffsets/TileByteCounts), since the
    /// three strip tags are replaced by four tile tags.
    static constexpr std::size_t entryCount =
        std::tuple_size_v<std::tuple<StaticEntry<256, 4, 1>, /* ImageWidth  */
                                     StaticEntry<257, 4, 1>, /* ImageLength */
                                     StaticEntry<258, 3, 1>, /* BitsPerSample */
                                     StaticEntry<259, 4, 1>, /* Compression */
                                     StaticEntry<262, 4, 1>, /* Photometric */
                                     StaticEntry<277, 4, 1>, /* SamplesPerPixel */
                                     StaticEntry<339, 3, 1>, /* SampleFormat */
                                     StaticEntry<278, 4, 1>, /* RowsPerStrip (strip) */
                                     StaticEntry<273, 4, 1>, /* StripOffsets (strip) */
                                     StaticEntry<279, 4, 1> /* StripByteCounts (strip) */>> +
        (isTiled ? 1 : 0);
};

// ---------------------------------------------------------------------------
// Runtime materialization: turns the compiled-in policy directory into the existing
// TiffIfdEntryToWrite vector and writes it through the shared writeTiffHeader/writeTiffIfd, so
// the emitted bytes are byte-identical to the runtime planTiffWrite path for the same layout.
// ---------------------------------------------------------------------------

template <typename Pixel,
          PartitionPolicy Partition,
          typename Compression,
          ContainerPolicy Container>
struct TiffPolicyWriter {
    static_assert(policy::PixelPolicy<Pixel>, "TiffPolicyWriter: Pixel is not a PixelPolicy");
    static_assert(policy::CompressionPolicy<Compression>,
                  "TiffPolicyWriter: Compression is not a CompressionPolicy");

    static constexpr bool isTiled = Partition::isTiled;
    using Directory = PolicyDirectory<Pixel, Partition, Compression, Container>;

    /// Builds the IFD entry vector (existing TiffIfdEntryToWrite form) for the given image
    /// geometry, and returns it together with the absolute file offset at which pixel data must
    /// begin (header + exact IFD size -- computed via the shared runtime tiffIfdByteSize so the
    /// compile-time and runtime writers agree).
    [[nodiscard]] static Result<std::pair<std::vector<TiffIfdEntryToWrite>, std::uint64_t>>
    plan(std::uint32_t imageWidth, std::uint32_t imageHeight) {
        using P = Pixel;
        using C = Compression;

        std::vector<TiffIfdEntryToWrite> entries;

        const auto push = [&](std::uint16_t entryTag,
                              ptiff::io::backend::tiff::FieldType fieldType,
                              std::vector<std::uint32_t> values) {
            entries.emplace_back(TiffIfdEntryToWrite{entryTag, fieldType, std::move(values)});
        };
        const auto tag = [](TagId id) { return static_cast<std::uint16_t>(id); };
        const auto ft =
            +[](std::uint16_t v) { return static_cast<ptiff::io::backend::tiff::FieldType>(v); };

        push(tag(TagId::ImageWidth), ft(4), {imageWidth});
        push(tag(TagId::ImageLength), ft(4), {imageHeight});
        push(tag(TagId::BitsPerSample), ft(3), {P::bitsPerSample});
        push(tag(TagId::Compression), ft(4), {C::kTagValue});
        push(tag(TagId::PhotometricInterpretation), ft(4), {kPhotometricGrayscale});
        push(tag(TagId::SamplesPerPixel), ft(4), {kSamplesPerPixel});
        push(tag(TagId::SampleFormat), ft(3), {P::sampleFormat});

        std::uint64_t dataBytesPerTile = 0;
        std::uint64_t tileCount = 0;

        if constexpr (isTiled) {
            const std::uint32_t cols = Partition::columns(imageWidth);
            const std::uint32_t rows = Partition::rows(imageHeight);
            tileCount = static_cast<std::uint64_t>(cols) * rows;
            const std::uint64_t tileRowBytes = static_cast<std::uint64_t>(Partition::tileWidth) *
                                               kSamplesPerPixel * P::bytesPerSample;
            dataBytesPerTile = tileRowBytes * Partition::tileHeight;

            push(tag(TagId::TileWidth), ft(4), {Partition::tileWidth});
            push(tag(TagId::TileLength), ft(4), {Partition::tileHeight});
            push(tag(TagId::TileOffsets), ft(4), std::vector<std::uint32_t>(tileCount, 0));
            push(tag(TagId::TileByteCounts),
                 ft(4),
                 std::vector<std::uint32_t>(tileCount,
                                            static_cast<std::uint32_t>(dataBytesPerTile)));
        } else {
            const std::uint64_t rowBytes =
                static_cast<std::uint64_t>(imageWidth) * kSamplesPerPixel * P::bytesPerSample;
            const std::uint64_t stripByteCount = rowBytes * imageHeight;

            push(tag(TagId::RowsPerStrip), ft(4), {imageHeight});
            push(tag(TagId::StripOffsets), ft(4), {0}); // patched below
            push(tag(TagId::StripByteCounts), ft(4), {static_cast<std::uint32_t>(stripByteCount)});
        }

        // Data offset = header + exact IFD size, reusing the shared runtime size routine so both
        // writers emit identical files.
        const std::uint64_t dataOffset =
            Container::headerSize + tiffIfdByteSize(entries, Container::isBigTiff);

        if constexpr (isTiled) {
            const std::uint32_t cols = Partition::columns(imageWidth);
            const std::uint32_t rows = Partition::rows(imageHeight);
            tileCount = static_cast<std::uint64_t>(cols) * rows;
            const std::uint64_t tileRowBytes = static_cast<std::uint64_t>(Partition::tileWidth) *
                                               kSamplesPerPixel * P::bytesPerSample;
            dataBytesPerTile = tileRowBytes * Partition::tileHeight;
            std::vector<std::uint32_t> offsets(tileCount);
            for (std::uint64_t i = 0; i < tileCount; ++i) {
                offsets[i] = static_cast<std::uint32_t>(dataOffset + i * dataBytesPerTile);
            }
            for (auto& e : entries) {
                if (e.tagId == tag(TagId::TileOffsets)) {
                    e.values = offsets;
                }
            }
        } else {
            for (auto& e : entries) {
                if (e.tagId == tag(TagId::StripOffsets)) {
                    e.values = {static_cast<std::uint32_t>(dataOffset)};
                }
            }
        }

        return std::make_pair(std::move(entries), dataOffset);
    }

    /// Serializes a complete TIFF file (header + IFD) for the given image geometry and returns
    /// the absolute offset at which the caller must write pixel data. Reuses writeTiffHeader and
    /// writeTiffIfd, so output is byte-identical to the runtime writer for the same layout.
    [[nodiscard]] static Result<std::uint64_t>
    writeDirectory(BinaryWriter& writer, std::uint32_t imageWidth, std::uint32_t imageHeight) {
        auto planned = plan(imageWidth, imageHeight);
        if (!planned.has_value()) {
            return std::unexpected(planned.error());
        }
        auto [entries, dataOffset] = std::move(*planned);
        auto headerOk = writeTiffHeader(writer, dataOffset, Container::isBigTiff);
        if (!headerOk.has_value()) {
            return std::unexpected(headerOk.error());
        }
        auto ifdOk = writeTiffIfd(writer, std::move(entries), Container::isBigTiff);
        if (!ifdOk.has_value()) {
            return std::unexpected(ifdOk.error());
        }
        return dataOffset;
    }
};

} // namespace ptiff::io::backend::tiff::policy
