#include <cstddef>
#include <memory>
#include <span>
#include <utility>
#include <vector>

#include <ptiff/io/backend/tiff/checked_arithmetic.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_directory_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_header.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_image_sink.hpp>
#include <ptiff/io/backend/tiff/tiff_image_source.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io::backend {

std::string_view TiffBackend::name() const noexcept {
    return "tiff";
}

BackendCapabilities TiffBackend::capabilities() const noexcept {
    return {.supportsTiling = true,
            .supportsStreaming = false,
            .supportsRandomAccess = true,
            .supportsCloudStreaming = true};
}

namespace {

struct TiffDirectoryChain {
    std::vector<tiff::TiffDirectory> directories;
    bool isBigTiff = false;
};

/// Walks the whole IFD chain from the header's first directory, interpreting each into a
/// TiffDirectory (one per image, in file order). Uses `reader` starting from its current
/// position for the header, then follows each IFD's trailing next-IFD offset until 0. Rejects a
/// cyclic or non-advancing chain. The single-IFD files this backend has always written produce a
/// chain of length 1.
Result<TiffDirectoryChain> readDirectoryChain(BinaryReader& reader) {
    auto header = tiff::readTiffHeader(reader);
    if (!header.has_value()) {
        return std::unexpected(header.error());
    }
    auto fileSize = reader.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }

    std::vector<tiff::TiffDirectory> directories;
    std::uint64_t ifdOffset = header->firstIfdOffset;
    constexpr int kMaxDirectories = 100000;
    for (int i = 0; i < kMaxDirectories; ++i) {
        // The IFD offset field is untrusted (RFC-0001 §13): it must point at a structure that
        // actually lives inside the file. Validate the minimum footprint (the entry-count field,
        // 2 bytes classic / 8 bytes BigTIFF) against the file size before parsing, so an
        // out-of-range or wrapped firstIfdOffset/next-IFD offset is rejected with a clear error
        // instead of relying on a vague seek failure or an unbounded follow.
        const std::uint64_t countFieldSize = header->isBigTiff ? 8 : 2;
        auto minIfdEnd = tiff::checkedAddU64(ifdOffset, countFieldSize);
        if (!minIfdEnd.has_value() || *minIfdEnd > *fileSize) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "readDirectoryChain: IFD offset outside the file bounds"});
        }
        auto ifd = tiff::readTiffIfd(reader, ifdOffset, header->endian, header->isBigTiff);
        if (!ifd.has_value()) {
            return std::unexpected(ifd.error());
        }
        auto directory = tiff::interpretTiffIfd(*ifd);
        if (!directory.has_value()) {
            return std::unexpected(directory.error());
        }
        directory->endian = header->endian;
        directories.push_back(std::move(*directory));

        const std::uint64_t next = ifd->nextIfdOffset();
        if (next == 0) {
            break;
        }
        if (next <= ifdOffset) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "readDirectoryChain: IFD chain does not advance (cyclic or corrupt)"});
        }
        ifdOffset = next;
    }
    if (directories.empty()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "readDirectoryChain: no IFD in file"});
    }
    return TiffDirectoryChain{std::move(directories), header->isBigTiff};
}

} // namespace

Result<std::unique_ptr<ImageSource>> TiffBackend::openImageSource(BinaryReader& reader) const {
    auto chain = readDirectoryChain(reader);
    if (!chain.has_value()) {
        return std::unexpected(chain.error());
    }
    return std::make_unique<tiff::TiffImageSource>(reader, std::move(chain->directories.front()));
}

Result<std::unique_ptr<ImageSink>> TiffBackend::openImageSink(BinaryWriter& writer,
                                                              const StorageModel& model) const {
    auto plan = tiff::planTiffWrite(model);
    if (!plan.has_value()) {
        return std::unexpected(plan.error());
    }
    auto seekResult = writer.seek(plan->directory.tileByteRanges.front().offset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<tiff::TiffImageSink>(writer, std::move(plan->directory));
}

Result<StorageModel> TiffBackend::deserializeModel(BinaryReader& reader) const {
    auto chain = readDirectoryChain(reader);
    if (!chain.has_value()) {
        return std::unexpected(chain.error());
    }
    StorageModel root;
    for (auto& directory : chain->directories) {
        root.addChild(tiff::toStorageModel(directory));
    }
    return root;
}

Result<std::unique_ptr<ImageSource>> TiffBackend::openImageSourceAt(BinaryReader& reader,
                                                                    std::size_t imageIndex) const {
    auto chain = readDirectoryChain(reader);
    if (!chain.has_value()) {
        return std::unexpected(chain.error());
    }
    if (imageIndex >= chain->directories.size()) {
        return std::unexpected(
            Error{ErrorCode::NotFound, "TiffBackend::openImageSourceAt: image index out of range"});
    }
    return std::make_unique<tiff::TiffImageSource>(reader,
                                                   std::move(chain->directories[imageIndex]));
}

Result<void> TiffBackend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    auto plan = tiff::planTiffWrite(model);
    if (!plan.has_value()) {
        return std::unexpected(plan.error());
    }
    const std::uint64_t headerSize =
        plan->isBigTiff ? tiff::kBigTiffHeaderSize : tiff::kClassicTiffHeaderSize;
    auto headerResult = tiff::writeTiffHeader(writer, headerSize, plan->isBigTiff);
    if (!headerResult.has_value()) {
        return std::unexpected(headerResult.error());
    }
    return tiff::writeTiffIfd(writer, std::move(plan->entries), plan->isBigTiff);
}

Result<void> TiffBackend::serializeModelList(std::span<const StorageModel> models,
                                             BinaryWriter& writer) const {
    auto plan = tiff::planTiffWriteMulti(models);
    if (!plan.has_value()) {
        return std::unexpected(plan.error());
    }
    const std::uint64_t headerSize =
        plan->isBigTiff ? tiff::kBigTiffHeaderSize : tiff::kClassicTiffHeaderSize;
    auto headerResult = tiff::writeTiffHeader(writer, headerSize, plan->isBigTiff);
    if (!headerResult.has_value()) {
        return std::unexpected(headerResult.error());
    }

    // Write the IFD chain contiguously after the header, each linked to the next via its
    // next-IFD offset; the last links to 0 (no following IFD). Pixel data is written later
    // through per-image sinks (openImageSinkAt), which seek to the rebased absolute offsets the
    // file plan already carries.
    std::uint64_t cursor = headerSize;
    for (std::size_t i = 0; i < plan->images.size(); ++i) {
        const std::uint64_t ifdSize =
            tiff::tiffIfdByteSize(plan->images[i].entries, plan->isBigTiff);
        const std::uint64_t nextIfdOffset = (i + 1 < plan->images.size()) ? cursor + ifdSize : 0;
        auto writeResult = tiff::writeTiffIfd(
            writer, std::move(plan->images[i].entries), plan->isBigTiff, nextIfdOffset);
        if (!writeResult.has_value()) {
            return std::unexpected(writeResult.error());
        }
        cursor += ifdSize;
    }
    return {};
}

Result<std::unique_ptr<ImageSink>> TiffBackend::openImageSinkAt(
    BinaryWriter& writer, std::span<const StorageModel> models, std::size_t imageIndex) const {
    auto plan = tiff::planTiffWriteMulti(models);
    if (!plan.has_value()) {
        return std::unexpected(plan.error());
    }
    if (imageIndex >= plan->images.size()) {
        return std::unexpected(
            Error{ErrorCode::OutOfRange, "TiffBackend::openImageSinkAt: image index out of range"});
    }
    auto seekResult = writer.seek(plan->images[imageIndex].directory.tileByteRanges.front().offset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<tiff::TiffImageSink>(writer,
                                                 std::move(plan->images[imageIndex].directory));
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("tiff", [] { return std::make_unique<TiffBackend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
