#pragma once

#include <cstdint>
#include <memory>
#include <span>
#include <string>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/binary_reader.hpp>

namespace ptiff::io {

/// @brief Concrete @ref ptiff::io::BinaryReader "BinaryReader" over a local file.
///
/// FileBinaryReader opens a local file read-only in binary mode and moves bytes through it with
/// the standard @ref ptiff::io::BinaryReader "BinaryReader" cursor API (read/seek/position/size).
/// It is backend-neutral -- any @ref ptiff::io::StorageBackend "StorageBackend" reading from
/// local files can use it, not just the TIFF backend.
///
/// @section file_binary_reader_thread Thread-safety
///
/// Not thread-safe, under the same contract as its
/// @ref ptiff::io::BinaryReader "BinaryReader" base class: it holds a mutable read cursor.
///
/// @section file_binary_reader_example Example
///
/// @code{.cpp}
/// using ptiff::io::FileBinaryReader;
///
/// auto r = FileBinaryReader::open("scene.ptiff");
/// assert(r.has_value());
/// assert(r.value()->size().value() > 0);
/// @endcode
///
/// @see @ref ptiff::io::BinaryReader "BinaryReader",
///      @ref ptiff::io::FileBinaryWriter "FileBinaryWriter".
class PTIFF_EXPORT FileBinaryReader final : public BinaryReader {
public:
    /// @brief Opens \p path read-only in binary mode.
    ///
    /// @param path The local filesystem path to open for reading.
    /// @return A `std::unique_ptr<FileBinaryReader>` on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if the file cannot be opened or its
    ///         size cannot be determined.
    ///
    /// @code{.cpp}
    /// auto r = FileBinaryReader::open("scene.ptiff");
    /// assert(r.has_value());
    /// @endcode
    [[nodiscard]] static Result<std::unique_ptr<FileBinaryReader>> open(const std::string& path);

    /// @brief Closes the underlying file stream.
    ~FileBinaryReader() override;

    /// @brief Reads up to `destination.size()` bytes into \p destination.
    /// @return The number of bytes actually read on success (less at end-of-input), or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a hard stream error.
    [[nodiscard]] Result<std::size_t> read(std::span<std::byte> destination) override;
    /// @brief Moves the read cursor to an absolute byte \p offset.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p offset is invalid
    ///         for the stream.
    [[nodiscard]] Result<void> seek(std::uint64_t offset) override;
    /// @brief Reports the current absolute byte offset of the read cursor.
    /// @return The current 0-based cursor position on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a stream error.
    [[nodiscard]] Result<std::uint64_t> position() const override;
    /// @brief Reports the file's total size in bytes.
    /// @return The file size captured at `open` time on success.
    [[nodiscard]] Result<std::uint64_t> size() const override;

private:
    FileBinaryReader();

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
