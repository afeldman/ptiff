#pragma once

#include <cstdint>
#include <memory>
#include <span>
#include <string>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/binary_reader.hpp>

namespace ptiff::io {

/// @brief Read-only @ref ptiff::io::BinaryReader "BinaryReader" over HTTP(S) Range requests.
///
/// HttpRangeBinaryReader fetches bytes from a remote object (S3/GCS/Azure Blob presigned URL, or
/// any HTTPS endpoint that honours the `Range` header) via libcurl, exposing the same cursor API
/// as @ref ptiff::io::FileBinaryReader "FileBinaryReader". Any @ref ptiff::io::StorageBackend
/// "StorageBackend" that only knows @ref ptiff::io::BinaryReader "BinaryReader" (e.g.
/// `TiffBackend` reading a Cloud-Optimized TIFF) works over this transport unmodified.
///
/// A single read-ahead buffer (default 64 KiB) absorbs the many small sequential reads a TIFF
/// header/IFD parse makes, so opening a remote file does not cost one HTTP request per field. A
/// single read larger than the buffer bypasses it entirely (served directly, buffer left
/// untouched) so one oversized tile fetch cannot evict the header region a later small read needs.
///
/// @section http_range_binary_reader_thread Thread-safety
///
/// Not thread-safe, under the same contract as its
/// @ref ptiff::io::BinaryReader "BinaryReader" base class: it holds a mutable read cursor and a
/// non-thread-safe libcurl "easy" handle.
///
/// @section http_range_binary_reader_example Example
///
/// @code{.cpp}
/// using ptiff::io::HttpRangeBinaryReader;
///
/// auto r = HttpRangeBinaryReader::open("https://example-bucket.s3.amazonaws.com/scene.tif?...");
/// assert(r.has_value());
/// assert(r.value()->size().value() > 0);
/// @endcode
///
/// @see @ref ptiff::io::BinaryReader "BinaryReader",
///      @ref ptiff::io::FileBinaryReader "FileBinaryReader" (local-file sibling).
class PTIFF_EXPORT HttpRangeBinaryReader final : public BinaryReader {
public:
    /// @brief Opens \p url for Range-read access.
    ///
    /// Issues one ranged `GET` (never `HEAD` -- presigned GET URLs commonly reject HEAD) to
    /// discover the object's total size and prime the read-ahead buffer.
    ///
    /// @param url         Must start with `http://` or `https://`. May already carry its own
    ///                    authorization as query parameters (a presigned URL).
    /// @param bearerToken If non-empty, sent as an `Authorization: Bearer <bearerToken>` header
    ///                    on every request.
    /// @return A `std::unique_ptr<HttpRangeBinaryReader>` on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" for a malformed \p url,
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" for an HTTP 404,
    ///         @ref ptiff::ErrorCode::Unknown "Unknown" for any other transport or HTTP-level
    ///         failure.
    [[nodiscard]] static Result<std::unique_ptr<HttpRangeBinaryReader>>
    open(const std::string& url, std::string_view bearerToken = {});

    /// @brief Releases the underlying libcurl handle.
    ~HttpRangeBinaryReader() override;

    /// @brief Reads up to `destination.size()` bytes into \p destination, fetching over HTTP as
    ///        needed.
    /// @return The number of bytes actually read on success (`0` at end-of-input), or an `Error`
    ///         on a transport/HTTP failure.
    [[nodiscard]] Result<std::size_t> read(std::span<std::byte> destination) override;
    /// @brief Moves the read cursor to an absolute byte \p offset. Lazy -- does not fetch.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p offset is beyond the size
    ///         discovered at `open()` time.
    [[nodiscard]] Result<void> seek(std::uint64_t offset) override;
    /// @brief Reports the current absolute byte offset of the read cursor.
    [[nodiscard]] Result<std::uint64_t> position() const override;
    /// @brief Reports the object's total size in bytes, as discovered at `open()` time.
    [[nodiscard]] Result<std::uint64_t> size() const override;

private:
    HttpRangeBinaryReader();

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
