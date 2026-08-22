#pragma once

// ISIS3-style file wrapper: a PDS3-like text label followed by a seekable pixel block.
//
//   [0 .. N)   ISIS3/PDS3-style text label (ends with a bare "End" line, exactly N bytes)
//   [N ..)     the image pixel block (tile-addressable, identical addressing to the memory
//              backend/memory backend pixel geometry)
//
// The pixel block is addressed by the label's on-disk length: because the label is a fixed text
// block terminated by a bare "End" marker, the pixel origin is deterministically the byte right
// after that marker (no circular dependency on a stored offset, no binary seek header). This is
// deliberately a compact, self-describing subset of ISIS3 (see ROADMAP M3) -- it mirrors the
// structure (Object=IsisCube -> Object=Core -> Group=Dimensions/Pixels) without implementing the
// full USGS data dictionary.

#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::isis {

/// @brief Serializes a flat per-image @p model into an ISIS3-style PDS3 text label.
/// @return The label bytes (including the trailing "End\n" marker) on success, or
///         `InvalidArgument` if a required storage field is absent.
[[nodiscard]] Result<std::vector<std::byte>> writeLabel(const StorageModel& model);

/// @brief Parses an ISIS3-style label into the flat per-image @ref StorageModel "StorageModel"
///        that produced it (round-trips with @ref writeLabel).
/// @param labelText The label bytes as read from the start of the document, including the
///        trailing "End" marker (the scanner in @c readDocument determines its extent).
/// @return The reconstructed model on success, or `InvalidArgument` if the bytes are not a
///         recognizable ISIS3 label / miss a required field.
[[nodiscard]] Result<StorageModel> parseLabel(std::span<const std::byte> labelText);

} // namespace ptiff::io::backend::isis
