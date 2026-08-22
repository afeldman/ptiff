#pragma once

#include <memory>
#include <span>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/backend_capabilities.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io {

/// @brief The seam between format-neutral layers and one concrete file format.
///
/// StorageBackend is the interface through which the format-neutral layers above
/// (@ref ptiff::io::Serializer "Serializer" / @ref ptiff::io::Deserializer "Deserializer",
/// @ref ptiff::io::StorageModel "StorageModel", @ref ptiff::io::BinaryReader "BinaryReader",
/// @ref ptiff::io::BinaryWriter "BinaryWriter") and one concrete file format below talk to each
/// other. It couples the byte transport (given to it as a BinaryReader / BinaryWriter) with the
/// mapping between a @ref ptiff::io::StorageModel "StorageModel" and that format's structure, and
/// with tile-level pixel access (@ref ptiff::io::ImageSource "ImageSource" /
/// @ref ptiff::io::ImageSink "ImageSink").
///
/// @section storage_backend_extension The extension point
///
/// A new format is a new StorageBackend subclass registered with
/// @ref ptiff::io::BackendFactory "BackendFactory" -- nothing above this layer changes.
///
/// @section storage_backend_thread Thread-safety
///
/// Thread-compatible: one instance is not shared across concurrent operations in this design
/// (each open file gets its own); stateless backends may relax this, documented per subclass.
///
/// @section storage_backend_example Example
///
/// @code{.cpp}
/// using ptiff::io::StorageBackend;
///
/// // `backend` was created via BackendFactory::create("tiff").
/// assert(backend->name() == "tiff");
/// auto caps = backend->capabilities();
/// assert(caps.supportsTiling);
/// @endcode
///
/// @see @ref ptiff::io::BackendFactory "BackendFactory",
///      @ref ptiff::io::BackendCapabilities "BackendCapabilities",
///      @ref ptiff::io::BinaryReader "BinaryReader".
class PTIFF_EXPORT StorageBackend {
public:
    virtual ~StorageBackend() = default;
    StorageBackend(const StorageBackend&) = delete;
    StorageBackend& operator=(const StorageBackend&) = delete;
    StorageBackend(StorageBackend&&) = delete;
    StorageBackend& operator=(StorageBackend&&) = delete;

    /// @brief Returns the canonical name of this backend ("tiff", "memory", ...).
    /// @return A `std::string_view` naming this backend; used as its registry key in
    ///         @ref ptiff::io::BackendFactory "BackendFactory".
    [[nodiscard]] virtual std::string_view name() const noexcept = 0;
    /// @brief Reports which capabilities this backend supports.
    /// @return The backend's @ref ptiff::io::BackendCapabilities "BackendCapabilities".
    [[nodiscard]] virtual BackendCapabilities capabilities() const noexcept = 0;

    /// @brief Opens an @ref ptiff::io::ImageSource "ImageSource" over \p reader.
    ///
    /// @param reader The byte transport to read the image from.
    /// @return A tile-level read handle on success, or a backend-specific error if the image
    ///         cannot be opened / parsed from \p reader.
    [[nodiscard]] virtual Result<std::unique_ptr<ImageSource>>
    openImageSource(BinaryReader& reader) const = 0;
    /// @brief Opens an @ref ptiff::io::ImageSink "ImageSink" over \p writer.
    ///
    /// @param writer The byte transport to write the image to.
    /// @param model  The format-neutral model describing the image to write.
    /// @return A tile-level write handle on success, or a backend-specific error if the sink
    ///         cannot be prepared from \p writer / \p model.
    [[nodiscard]] virtual Result<std::unique_ptr<ImageSink>>
    openImageSink(BinaryWriter& writer, const StorageModel& model) const = 0;

    /// @brief Reads a document's format-neutral model from \p reader.
    ///
    /// @param reader The byte transport to read from.
    /// @return The parsed @ref ptiff::io::StorageModel "StorageModel" on success, or a
    ///         backend-specific error if the bytes cannot be interpreted.
    [[nodiscard]] virtual Result<StorageModel> deserializeModel(BinaryReader& reader) const = 0;
    /// @brief Writes \p model to \p writer in this backend's format.
    ///
    /// @param model  The format-neutral model to persist.
    /// @param writer The byte transport to write to.
    /// @return `Result<void>` success on success, or a backend-specific error if the model cannot
    ///         be encoded / written.
    [[nodiscard]] virtual Result<void> serializeModel(const StorageModel& model,
                                                      BinaryWriter& writer) const = 0;

    /// @brief Writes several images (one per model) to \p writer as a single multi-image file.
    ///
    /// Multi-image aware backends (e.g. TIFF's IFD chain) write \p models in order as one file;
    /// backends that only understand a single image return
    /// @ref ptiff::ErrorCode::NotImplemented "NotImplemented". `serializeModel` on a single
    /// image is equivalent to this with `models.size() == 1`, so callers may use either path.
    ///
    /// @param models The format-neutral models to persist, in file (chain) order.
    /// @param writer The byte transport to write to.
    /// @return `Result<void>` success on success, or a backend-specific error.
    [[nodiscard]] virtual Result<void> serializeModelList(std::span<const StorageModel> models,
                                                          BinaryWriter& writer) const {
        (void)models;
        (void)writer;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "StorageBackend: serializeModelList() not supported"});
    }

    /// @brief Opens an @ref ptiff::io::ImageSink "ImageSink" for one image of a multi-image file.
    ///
    /// Multi-image aware backends return a sink positioned for the `imageIndex`-th image's data
    /// region; single-image backends return @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @param writer     The byte transport to write to.
    /// @param models     The full set of models to persist; the sink uses `models[imageIndex]`.
    /// @param imageIndex Which image's data region the sink should address.
    /// @return A tile-level write handle on success, or a backend-specific error.
    [[nodiscard]] virtual Result<std::unique_ptr<ImageSink>> openImageSinkAt(
        BinaryWriter& writer, std::span<const StorageModel> models, std::size_t imageIndex) const {
        (void)writer;
        (void)models;
        (void)imageIndex;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "StorageBackend: openImageSinkAt() not supported"});
    }

    /// @brief Opens an @ref ptiff::io::ImageSource "ImageSource" for one image of a multi-image
    ///        file.
    ///
    /// Multi-image aware backends return a source for the `imageIndex`-th directory of the IFD
    /// chain; single-image backends return @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @param reader     The byte transport to read from.
    /// @param imageIndex Which directory's image the source should expose.
    /// @return A tile-level read handle on success, or a backend-specific error.
    [[nodiscard]] virtual Result<std::unique_ptr<ImageSource>>
    openImageSourceAt(BinaryReader& reader, std::size_t imageIndex) const {
        (void)reader;
        (void)imageIndex;
        return std::unexpected(
            Error{ErrorCode::NotImplemented, "StorageBackend: openImageSourceAt() not supported"});
    }

protected:
    StorageBackend() = default;
};

} // namespace ptiff::io
