#pragma once

// PDS4 file wrapper: an embedded PDS4-style label plus a seekable pixel block.
//
// Layout of a PDS4 document produced/consumed by the Pds4Backend:
//
//   [0 .. 8)       label byte length N, little-endian uint64 (the 'seek header')
//   [8 .. 8+N)     the PDS4 label (UTF-8 XML, exactly N bytes)
//   [8+N ..)       the image pixel block (tile-addressable, see imageInfoFromModel)
//
// The seek header is the only non-XML part and exists so the absolute pixel offsets
// stay deterministic without circularly depending on the label's own text length
// (the same mechanism the memory backend uses for its document header). The label
// itself follows the broad PDS4 structural conventions (a Product_Observational root
// holding one Array_2D_Image per image) and carries the format-neutral storage fields.
//
// This is a deliberately small, self-describing subset of PDS4 (see ROADMAP M3
// "no PDS4-archival replacement"); it is not a full PDS4 catalogue product.

#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::pds4 {

/// @brief Byte length of the seek header that precedes the XML label.
inline constexpr std::uint64_t kHeaderSize = sizeof(std::uint64_t);

/// @brief Serializes a flat per-image @p model into a PDS4-style XML label byte block.
/// @return The label text as bytes (not including the seek header) on success, or
///         `InvalidArgument` if a required storage field is absent.
[[nodiscard]] Result<std::vector<std::byte>> writeLabel(const StorageModel& model);

/// @brief Parses a PDS4-style XML label into the flat per-image @ref StorageModel "StorageModel"
///        that produced it (round-trips with @ref writeLabel).
/// @param labelBytes The label bytes read from offset `kHeaderSize` in the document.
/// @return The reconstructed model on success, or `InvalidArgument` if the bytes are not a
///         recognizable PDS4 label / miss a required field.
[[nodiscard]] Result<StorageModel> parseLabel(std::span<const std::byte> labelBytes);

} // namespace ptiff::io::backend::pds4
