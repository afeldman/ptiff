#pragma once

/// @file reader.hpp
/// @brief `ptiff::Reader` — read facade over the C-ABI.
///
/// `Reader::open(path)` opens a PTIFF file and returns a `std::unique_ptr<Reader>`
/// (or an `Error`). `scene()` deserializes the file's geometry/storage metadata
/// into an owned `Scene`; `imageSource(imageId)` gives read-side pixel access
/// as a `std::unique_ptr<Source>`.

#include <memory>
#include <string>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp>
#include <ptiff/scene.hpp>
#include <ptiff/source.hpp>

namespace ptiff {

class Reader {
public:
    ~Reader() = default;
    Reader(const Reader&) = delete;
    Reader& operator=(const Reader&) = delete;

    /// @brief Opens `path` and returns a backend-selected `Reader`.
    ///
    /// @return A `std::unique_ptr<Reader>` on success, or
    ///         `ErrorCode::NotFound` if the file cannot be located/opened.
    static Result<std::unique_ptr<Reader>> open(const std::string& path) {
        auto r = std::unique_ptr<Reader>(new Reader(path));
        // Validate by opening the source handle eagerly: a missing/invalid
        // file must surface here, not lazily at scene()/imageSource().
        int32_t err = 0;
        detail::ptiff_source* probe = detail::ptiff_source_open(path.c_str(), &err);
        if (!probe) {
            return std::unexpected(Error{cErrorCodeToEnum(err),
                                            "Reader::open: cannot open path"});
        }
        detail::ptiff_source_close(probe);
        return r;
    }

    /// @brief Deserializes the file into an owned `Scene` (one image).
    ///
    /// The current C-ABI slice exposes the first IFD/image; multi-image files
    /// are stitched via `imageSource` on subsequent ids when the backend
    /// supports them.
    Result<Scene> scene() const {
        detail::ptiff_image_descriptor c{};
        int32_t rc = detail::ptiff_open_path(path_.c_str(), &c);
        if (rc != 0) {
            return std::unexpected(Error{cErrorCodeToEnum(rc),
                                            "Reader::scene: open_path failed"});
        }
        Scene s;
        auto r = s.addImage(detail::fromCDescriptor(c));
        if (!r) return std::unexpected(r.error());
        return s;
    }

    /// @brief Returns read-side pixel access to one image.
    [[nodiscard]] Result<std::unique_ptr<Source>> imageSource(ImageId imageId) {
        if (imageId.value() != 0) {
            return std::unexpected(Error{ErrorCode::NotFound,
                                            "Reader::imageSource: only id 0 is exposed"});
        }
        int32_t err = 0;
        Source* src = Source::open(path_, &err);
        if (!src) {
            return std::unexpected(Error{cErrorCodeToEnum(err),
                                            "Reader::imageSource: source open failed"});
        }
        return std::unique_ptr<Source>(src);
    }

private:
    explicit Reader(std::string path) : path_(std::move(path)) {}
    std::string path_;
};

} // namespace ptiff
