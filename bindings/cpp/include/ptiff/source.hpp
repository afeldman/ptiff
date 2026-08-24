#pragma once

/// @file source.hpp
/// @brief `ptiff::Source` — read-side pixel access over the C-ABI source
///        handle (`ptiff_source_*`).
///
/// A `Source` exposes one image's decoded pixel tiles in a format-agnostic
/// way: a tile grid (`tileColumns`/`tileRows`), a uniform decoded `tileBytes`,
/// and `readTile(column, row)` returning a `std::vector<uint8_t>`. It is the
/// read-side mirror of `Writer::write(scene, provider)`.

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp>
#include <ptiff/image.hpp>

namespace ptiff {

/// @brief RAII read handle over one image's pixel tiles.
class Source {
public:
    /// @brief Opens the image at `path` for tile reads.
    ///
    /// Returns `nullptr` (with `err_out` set) on failure.
    static Source* open(const std::string& path, int32_t* err_out) {
        auto* h = detail::ptiff_source_open(path.c_str(), err_out);
        if (!h) return nullptr;
        return new Source(h);
    }

    ~Source() { close(); }
    Source(const Source&) = delete;
    Source& operator=(const Source&) = delete;

    [[nodiscard]] ImageDescriptor descriptor() const noexcept {
        detail::ptiff_image_descriptor c{};
        detail::ptiff_source_descriptor(h_, &c);
        return detail::fromCDescriptor(c);
    }
    [[nodiscard]] std::uint32_t tileColumns() const noexcept {
        return detail::ptiff_source_tile_columns(h_);
    }
    [[nodiscard]] std::uint32_t tileRows() const noexcept {
        return detail::ptiff_source_tile_rows(h_);
    }
    [[nodiscard]] std::size_t tileBytes() const noexcept {
        return static_cast<std::size_t>(detail::ptiff_source_tile_byte_size(h_));
    }

    /// @brief Reads decoded tile `(column, row)`.
    ///
    /// Returns the tile bytes, or an error describing the failure.
    [[nodiscard]] Result<std::vector<uint8_t>> readTile(std::uint32_t column,
                                                        std::uint32_t row) const {
        std::size_t n = tileBytes();
        if (n == 0) {
            return std::unexpected(Error{ErrorCode::Unknown,
                                            "Source::readTile: no source bytes"});
        }
        std::vector<uint8_t> buf(n);
        uintptr_t bytes_read = 0;
        int32_t rc = detail::ptiff_source_read_tile(h_, column, row, buf.data(),
                                                    static_cast<uintptr_t>(buf.size()), &bytes_read);
        if (rc != 0) {
            return std::unexpected(Error{cErrorCodeToEnum(rc),
                                            "Source::readTile: C-ABI read failed"});
        }
        buf.resize(static_cast<std::size_t>(bytes_read));
        return buf;
    }

private:
    explicit Source(detail::ptiff_source* h) : h_(h) {}
    void close() noexcept {
        if (h_) {
            detail::ptiff_source_close(h_);
            h_ = nullptr;
        }
    }
    detail::ptiff_source* h_;
};

} // namespace ptiff
