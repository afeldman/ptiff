#pragma once

#include <memory>
#include <string_view>

#include <ptiff/core/id.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

/// @brief Abstract, backend-agnostic read interface for a container image file.
///
/// Reader is the top-level, format- and backend-agnostic handle for reading a PTIFF/TIFF/
/// BigTIFF (or other container) file. Concrete backends -- memory-mapped, buffered stream,
/// cloud range-reads, ... -- live behind `Reader::open()`, so adding a new backend or format
/// never requires changing this header or the callers that hold a `Reader`.
///
/// @section reader_factory Backend selection via factory
///
/// The only way to obtain a Reader is the static `open()` factory, which selects a concrete
/// backend (typically through @ref ptiff::io::BackendFactory "BackendFactory") based on the
/// file's path/signature. Ownership is returned as a `std::unique_ptr`, so a Reader is moveable
/// but never copyable.
///
/// @section reader_example Example
///
/// @code{.cpp}
/// using ptiff::Reader;
///
/// auto reader = Reader::open("scene.ptiff");
/// if (reader.has_value()) {
///     // reader is a std::unique_ptr<Reader> of some concrete backend.
///     assert(static_cast<bool>(*reader));
/// } else {
///     assert(reader.error().code() == ptiff::ErrorCode::NotFound);
/// }
/// @endcode
///
/// @see @ref ptiff::Writer "Writer" (write mirror),
///      @ref ptiff::io::BackendFactory "BackendFactory".
class PTIFF_EXPORT Reader {
public:
    virtual ~Reader() = default;

    Reader(const Reader&) = delete;
    Reader& operator=(const Reader&) = delete;
    Reader(Reader&&) = delete;
    Reader& operator=(Reader&&) = delete;

    /// @brief Opens \p path and returns a backend-selected Reader.
    ///
    /// @param path A local filesystem path, or a `http://`/`https://` URL for a remote object
    ///             read via @ref ptiff::io::HttpRangeBinaryReader "HttpRangeBinaryReader"
    ///             (Range requests). A `http(s)://` prefix selects the HTTP transport; anything
    ///             else is treated as a local path.
    /// @param bearerToken Only meaningful when \p path is a `http(s)://` URL: if non-empty, sent
    ///             as an `Authorization: Bearer <bearerToken>` header on every request. Ignored
    ///             for local paths.
    /// @return A `std::unique_ptr<Reader>` backed by the selected concrete backend on success,
    ///         or @ref ptiff::ErrorCode::NotFound "NotFound" if the file cannot be located or
    ///         opened, or another backend-specific error otherwise.
    ///
    /// @code{.cpp}
    /// auto r = Reader::open("scene.ptiff");
    /// assert(r.has_value());
    /// @endcode
    static Result<std::unique_ptr<Reader>> open(std::string_view path,
                                                std::string_view bearerToken = {});

    /// @brief Returns the scene deserialized from this file on read-back.
    ///
    /// Only meaningful after @ref open "open" succeeds and the backend produced a model. The
    /// scene is owned by the Reader and remains valid for the Reader's lifetime. Default
    /// implementations of derived classes that are not real readers return
    /// @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @return Pointer to the owned scene on success, or an error if the reader cannot provide
    ///         a deserialized scene.
    [[nodiscard]] virtual Result<const Scene*> scene() const {
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "Reader: scene() not supported by this reader"});
    }

    /// @brief Returns a handle to one image's pixel data.
    ///
    /// This is the read-side mirror of @ref ptiff::Writer::write "Writer::write(scene, provider)":
    /// it exposes one image's pixels through the format-agnostic
    /// @ref ptiff::io::ImageSource "ImageSource" interface (`layout()` + `readTile()`), so a
    /// caller can walk an image's tiles without touching the backend or the byte transport.
    ///
    /// @param imageId Which image's pixels to read, in the @ref ptiff::Scene "Scene"'s image
    ///                order. The concrete backend decides which ids it can honour; today's TIFF
    ///                backend exposes one image per IFD in the chain, so ids 0..N-1 are valid
    ///                for an N-image file.
    /// @return The image's @ref ptiff::io::ImageSource "ImageSource" on success. The source is
    ///         owned by the caller but its byte transport is shared with this Reader, so it is
    ///         only valid while this Reader outlives it. Returns
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p imageId does not name an image
    ///         the reader can produce, or a backend-specific error if the image cannot be
    ///         opened.
    ///
    /// @code{.cpp}
    /// auto reader = ptiff::Reader::open("scene.ptiff");
    /// auto source = (*reader)->imageSource(ptiff::ImageId{0});
    /// if (source.has_value()) {
    ///     const auto& layout = (*source)->layout();
    ///     auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{0, 0, 0});
    ///     assert(tile.has_value());
    /// }
    /// @endcode
    ///
    /// @see @ref ptiff::io::ImageSource "ImageSource",
    ///      @ref ptiff::io::StorageBackend::openImageSourceAt "StorageBackend::openImageSourceAt".
    [[nodiscard]] virtual Result<std::unique_ptr<io::ImageSource>> imageSource(ImageId imageId) {
        (void)imageId;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "Reader: imageSource() not supported by this reader"});
    }

protected:
    Reader() = default;
};

} // namespace ptiff
