#pragma once

/// @file writer.hpp
/// @brief `ptiff::Writer` — write facade over the C-ABI sink.
///
/// `Writer::create(path)` is the write mirror of `Reader::open`. `write(scene,
/// provider)` persists the scene's metadata and pulls each pixel tile from a
/// caller-supplied `TileProvider` via `ptiff_sink_create` / `ptiff_sink_write_tile`.

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp>
#include <ptiff/image.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

/// @brief Mutable source of raw pixel tiles consumed by `Writer::write`.
///
/// A provider feeds one tile per `(column, row)` grid cell of a tiled image.
/// Implementations may reuse a buffer across calls (the writer copies).
class TileProvider {
public:
    virtual ~TileProvider() = default;
    virtual std::size_t tileBytes() const noexcept = 0;
    virtual bool tile(std::uint32_t column, std::uint32_t row,
                        std::vector<uint8_t>& out) = 0;
};

/// @brief RAII write handle over `ptiff_sink_*`.
class Writer {
public:
    ~Writer() { close(); }
    Writer(const Writer&) = delete;
    Writer& operator=(const Writer&) = delete;

    /// @brief Creates `path` for writing and returns a `Writer`.
    static Result<std::unique_ptr<Writer>> create(const std::string& path) {
        auto w = std::unique_ptr<Writer>(new Writer(path));
        return w;
    }

    /// @brief Writes a single image from `scene` together with its pixel data.
    ///
    /// `provider` feeds the image's tiles. The scene must contain exactly one
    /// image with a tile layout (`ImageDescriptor::tileInfo`), matching the
    /// C-ABI `ptiff_sink_create` requirement.
    Result<void> write(const Scene& scene, TileProvider& provider) {
        auto count = scene.imageCount();
        if (!count) return std::unexpected(count.error());
        if (*count != 1) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                            "Writer::write: scene must contain exactly one image"});
        }
        auto img = scene.imageAt(0);
        if (!img) return std::unexpected(img.error());
        const ImageDescriptor desc = ImageDescriptor{
            .width = img->get().width(),
            .height = img->get().height(),
            .pixelType = img->get().pixelType(),
            .channelCount = img->get().channelCount(),
            .groundSampleDistanceMeters = img->get().groundSampleDistanceMeters(),
            .tileInfo = img->get().tileInfo(),
            .compression = img->get().compression(),
        };
        if (!desc.tileInfo.has_value()) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                            "Writer::write: image must be tiled"});
        }
        auto c = detail::toCDescriptor(desc);
        detail::ptiff_sink* sink = detail::ptiff_sink_create(path_.c_str(), &c);
        if (!sink) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                            "Writer::write: sink_create failed"});
        }
        handle_ = sink;
        const std::uint32_t cols = detail::ptiff_sink_tile_columns(sink);
        const std::uint32_t rows = detail::ptiff_sink_tile_rows(sink);
        const std::size_t bytes = static_cast<std::size_t>(detail::ptiff_sink_tile_byte_size(sink));
        for (std::uint32_t r = 0; r < rows; ++r) {
            for (std::uint32_t c2 = 0; c2 < cols; ++c2) {
                std::vector<uint8_t> tile;
                if (!provider.tile(c2, r, tile)) {
                    return std::unexpected(Error{ErrorCode::Unknown,
                                                    "Writer::write: provider tile error"});
                }
                if (tile.size() != bytes) {
                    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                                    "Writer::write: provider tile size mismatch"});
                }
                int32_t rc = detail::ptiff_sink_write_tile(sink, c2, r, tile.data(),
                                                            static_cast<uintptr_t>(tile.size()));
                if (rc != 0) {
                    return std::unexpected(Error{cErrorCodeToEnum(rc),
                                                    "Writer::write: sink_write_tile failed"});
                }
            }
        }
        // Flush; then release the handle so a second write() can re-open.
        close();
        return {};
    }

private:
    explicit Writer(std::string path) : path_(std::move(path)) {}
    void close() noexcept {
        if (handle_) {
            detail::ptiff_sink_close(handle_);
            handle_ = nullptr;
        }
    }
    std::string path_;
    detail::ptiff_sink* handle_ = nullptr;
};

} // namespace ptiff
