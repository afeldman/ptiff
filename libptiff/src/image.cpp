#include <utility>

#include <ptiff/image.hpp>

namespace ptiff {

struct Image::Impl {
    ImageDescriptor descriptor;
};

Image::Image(ImageDescriptor descriptor)
    : impl_(std::make_unique<Impl>(Impl{std::move(descriptor)})) {}

Image::~Image() = default;
Image::Image(Image&&) noexcept = default;
Image& Image::operator=(Image&&) noexcept = default;

std::uint32_t Image::width() const noexcept {
    return impl_->descriptor.width;
}

std::uint32_t Image::height() const noexcept {
    return impl_->descriptor.height;
}

PixelType Image::pixelType() const noexcept {
    return impl_->descriptor.pixelType;
}

std::uint32_t Image::channelCount() const noexcept {
    return impl_->descriptor.channelCount;
}

std::optional<double> Image::groundSampleDistanceMeters() const noexcept {
    return impl_->descriptor.groundSampleDistanceMeters;
}

std::optional<TileInfo> Image::tileInfo() const noexcept {
    return impl_->descriptor.tileInfo;
}

std::optional<CompressionKind> Image::compression() const noexcept {
    return impl_->descriptor.compression;
}

} // namespace ptiff
