#pragma once

#include <functional>
#include <memory>
#include <string_view>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile_provider.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

/// @brief Abstract, backend-agnostic write interface for a container image file.
///
/// Writer is the top-level, format- and backend-agnostic handle for writing a PTIFF/TIFF/
/// BigTIFF (or other container) file. It mirrors @ref ptiff::Reader "Reader"'s
/// backend-selection-via-factory design: a new backend or format is added behind the static
/// `create()` factory without changing this header or its callers.
///
/// Ownership is returned as a `std::unique_ptr`, so a Writer is moveable but never copyable.
///
/// @section writer_example Example
///
/// @code{.cpp}
/// using ptiff::Writer;
///
/// auto writer = Writer::create("out.ptiff");
/// if (writer.has_value()) {
///     assert(static_cast<bool>(*writer));
/// } else {
///     assert(writer.error().code() == ptiff::ErrorCode::InvalidArgument);
/// }
/// @endcode
///
/// @see @ref ptiff::Reader "Reader" (read mirror),
///      @ref ptiff::io::BackendFactory "BackendFactory".
class PTIFF_EXPORT Writer {
public:
    virtual ~Writer() = default;

    Writer(const Writer&) = delete;
    Writer& operator=(const Writer&) = delete;
    Writer(Writer&&) = delete;
    Writer& operator=(Writer&&) = delete;

    /// @brief Creates \p path for writing and returns a backend-selected Writer.
    ///
    /// @param path The filesystem path (or transport-identifying string) of the file to create.
    /// @return A `std::unique_ptr<Writer>` backed by the selected concrete backend on success,
    ///         or @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if the file cannot be
    ///         created (e.g. the containing directory doesn't exist), or another backend-specific
    ///         error otherwise.
    ///
    /// @code{.cpp}
    /// auto w = Writer::create("out.ptiff");
    /// assert(w.has_value());
    /// @endcode
    static Result<std::unique_ptr<Writer>> create(std::string_view path);

    /// @brief Writes a full @ref ptiff::Scene "Scene" to this file.
    ///
    /// Serializes \p scene to a @ref ptiff::io::StorageModel "StorageModel" and writes it
    /// through the underlying @ref ptiff::io::StorageBackend "StorageBackend". Default
    /// implementations of derived classes that are not real writers return
    /// @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @param scene The scene to persist.
    /// @return `Result<void>` success on success, or a serialize/backend error otherwise.
    [[nodiscard]] virtual Result<void> write(const Scene& scene) {
        (void)scene;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "Writer: write() not supported by this writer"});
    }

    /// @brief Writes a full @ref ptiff::Scene "Scene" together with its pixel data to this file.
    ///
    /// Like `write(scene)`, serializes \p scene to a
    /// @ref ptiff::io::StorageModel "StorageModel" and writes it through the underlying
    /// @ref ptiff::io::StorageBackend "StorageBackend"; additionally persists the raw pixel
    /// bytes by pulling each strip/tile from \p provider and handing it to the backend's
    /// @ref ptiff::io::ImageSink "ImageSink". The provider's @ref ptiff::io::tile::TileLayout
    /// "TileLayout" must agree with the layout the backend derives from \p scene, or the write
    /// fails with @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument". This single-provider
    /// form requires a single-image scene; for a multi-image scene use the collection overload
    /// `write(scene, providers)`.
    ///
    /// @param scene    The scene's geometry and storage metadata to persist.
    /// @param provider The mutable source of the raw pixel tiles (may reuse buffers; see
    ///                 @ref ptiff::io::TileProvider "TileProvider" for the byte-count contract).
    /// @return `Result<void>` success on success, or a serialize/backend/IO error otherwise.
    [[nodiscard]] virtual Result<void> write(const Scene& scene, io::TileProvider& provider) {
        (void)scene;
        (void)provider;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "Writer: write(scene, provider) not supported"});
    }

    /// @brief Writes a full, possibly multi-image @ref ptiff::Scene "Scene" together with each
    ///        image's pixel data.
    ///
    /// Like `write(scene, provider)`, serializes \p scene to
    /// @ref ptiff::io::StorageModel "StorageModel"s and writes them through the underlying
    /// @ref ptiff::io::StorageBackend "StorageBackend" as a single multi-image file; `providers`
    /// carries one @ref ptiff::io::TileProvider "TileProvider" per image, in the scene's image
    /// order, each feeding that image's strips/tiles to the backend's
    /// @ref ptiff::io::ImageSink "ImageSink". `providers.size()` must equal the scene's image
    /// count, and each provider's @ref ptiff::io::tile::TileLayout "TileLayout" must agree with
    /// the layout the backend derives from that image's metadata, or the write fails with
    /// @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument".
    ///
    /// @param scene     The scene to persist.
    /// @param providers One provider per image, in the scene's image order.
    /// @return `Result<void>` success on success, or a serialize/backend/IO error otherwise.
    [[nodiscard]] virtual Result<void>
    write(const Scene& scene,
          const std::vector<std::reference_wrapper<io::TileProvider>>& providers) {
        (void)scene;
        (void)providers;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "Writer: write(scene, providers) not supported"});
    }

protected:
    Writer() = default;
};

} // namespace ptiff
