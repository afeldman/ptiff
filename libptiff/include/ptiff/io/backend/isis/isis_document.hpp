#pragma once

// Helpers shared by the IsisBackend methods: scan a PDS3 label from the start of the document
// and report where its pixel block begins. See isis_label.hpp for the byte layout.

#include <cstdint>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/isis/isis_label.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::isis {

/// @brief A scanned ISIS3 document plus the byte length of its label (the pixel origin sits
///        directly after the bare "End" marker).
struct IsisDocument {
    StorageModel image;
    std::uint64_t labelBytes = 0; // label text length (incl. trailing "End\n")
};

/// @brief Scans and parses an ISIS3 document from the start of @p reader.
/// @return @ref IsisDocument "IsisDocument" on success, or a backend-specific error on
///         malformed input (no label / no End marker / unparsable label).
[[nodiscard]] Result<IsisDocument> readDocument(io::BinaryReader& reader);

/// @brief Absolute byte offset of this image's pixel block (== the label byte length).
[[nodiscard]] inline std::uint64_t pixelOrigin(std::uint64_t labelBytes) {
    return labelBytes;
}

} // namespace ptiff::io::backend::isis
