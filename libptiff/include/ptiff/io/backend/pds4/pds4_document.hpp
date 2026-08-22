#pragma once

// Helpers shared by the Pds4Backend methods: read a PDS4 document's seek header + label, and
// derive the absolute pixel origin. See pds4_label.hpp for the byte layout.

#include <cstdint>
#include <utility>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/pds4/pds4_label.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::pds4 {

/// @brief A parsed PDS4 document plus the byte length of its XML label (needed to compute the
///        absolute pixel origin, which sits directly after the label).
struct Pds4Document {
    StorageModel image;           // flat per-image storage fields
    std::uint64_t labelBytes = 0; // length of the label text
};

/// @brief Reads a PDS4 document (seek header + label) from @p reader and parses it.
/// @return @ref Pds4Document "Pds4Document" on success, or a backend-specific error on
///         malformed input.
[[nodiscard]] Result<Pds4Document> readDocument(io::BinaryReader& reader);

/// @brief Absolute byte offset of this image's pixel block (seek header + label length).
[[nodiscard]] inline std::uint64_t pixelOrigin(std::uint64_t labelBytes) {
    return kHeaderSize + labelBytes;
}

} // namespace ptiff::io::backend::pds4
