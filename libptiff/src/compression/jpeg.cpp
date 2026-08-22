#include <ptiff/compression/jpeg.hpp>

// C API, confined to this translation unit -- never let libjpeg types reach the public header.
extern "C" {
#include <jpeglib.h>
}

#include <csetjmp>
#include <cstdlib>

namespace ptiff::compression {

namespace {

struct JpegErrorContext {
    jpeg_error_mgr pub;
    std::jmp_buf setjmpBuffer;
};

void jpegErrorExit(j_common_ptr cinfo) {
    auto* err = reinterpret_cast<JpegErrorContext*>(cinfo->err);
    std::longjmp(err->setjmpBuffer, 1);
}

} // namespace

Result<std::vector<std::byte>> encodeJpeg(std::span<const std::byte> pixels,
                                          std::uint32_t width,
                                          std::uint32_t height,
                                          std::uint32_t samplesPerPixel,
                                          int quality) {
    if (quality < 0 || quality > 100) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeJpeg: quality must be in [0, 100]"});
    }
    if (samplesPerPixel != 1 && samplesPerPixel != 3) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeJpeg: samplesPerPixel must be 1 or 3"});
    }
    const std::size_t expectedSize = static_cast<std::size_t>(width) * height * samplesPerPixel;
    if (pixels.size() != expectedSize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "encodeJpeg: pixels size does not match width * height * samplesPerPixel"});
    }

    jpeg_compress_struct cinfo{};
    JpegErrorContext errorContext{};
    cinfo.err = jpeg_std_error(&errorContext.pub);
    errorContext.pub.error_exit = jpegErrorExit;

    // No C++ object with a non-trivial destructor may be alive across this setjmp/longjmp
    // boundary (longjmp does not run destructors -- UB per [stmt.jump] otherwise). Only the
    // vector built after jpeg_finish_compress (all risky libjpeg calls done) is a std::vector;
    // everything touched between setjmp and that point is a C struct or a std::span (trivial).
    // outBuffer is not marked volatile: its address is passed to jpeg_mem_dest (which requires
    // a non-volatile unsigned char**), and that address-taking already forces the compiler to
    // keep it in memory rather than a register, so it survives longjmp correctly regardless.
    unsigned char* outBuffer = nullptr;
    unsigned long outSize = 0;
    if (setjmp(errorContext.setjmpBuffer)) {
        std::free(outBuffer);
        jpeg_destroy_compress(&cinfo);
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeJpeg: libjpeg-turbo compression failed"});
    }

    jpeg_create_compress(&cinfo);
    jpeg_mem_dest(&cinfo, &outBuffer, &outSize);

    cinfo.image_width = width;
    cinfo.image_height = height;
    cinfo.input_components = static_cast<int>(samplesPerPixel);
    cinfo.in_color_space = samplesPerPixel == 1 ? JCS_GRAYSCALE : JCS_RGB;
    jpeg_set_defaults(&cinfo);
    jpeg_set_quality(&cinfo, quality, TRUE);
    // 4:4:4: no chroma subsampling, every component sampled 1x1.
    for (int i = 0; i < cinfo.num_components; ++i) {
        cinfo.comp_info[i].h_samp_factor = 1;
        cinfo.comp_info[i].v_samp_factor = 1;
    }

    jpeg_start_compress(&cinfo, TRUE);
    const std::size_t rowStride = static_cast<std::size_t>(width) * samplesPerPixel;
    while (cinfo.next_scanline < cinfo.image_height) {
        // libjpeg's write API takes a non-const JSAMPROW despite not modifying the row data.
        JSAMPROW rowPointer[1] = {const_cast<JSAMPROW>(reinterpret_cast<const JSAMPLE*>(
            pixels.data() + static_cast<std::size_t>(cinfo.next_scanline) * rowStride))};
        if (jpeg_write_scanlines(&cinfo, rowPointer, 1) != 1) {
            jpeg_abort_compress(&cinfo);
            jpeg_destroy_compress(&cinfo);
            std::free(outBuffer);
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "encodeJpeg: libjpeg-turbo failed to write scanline"});
        }
    }
    jpeg_finish_compress(&cinfo);
    jpeg_destroy_compress(&cinfo);

    std::vector<std::byte> output(reinterpret_cast<std::byte*>(outBuffer),
                                  reinterpret_cast<std::byte*>(outBuffer) + outSize);
    std::free(outBuffer); // jpeg_mem_dest allocates with malloc; free with free(), not delete.
    return output;
}

Result<std::vector<std::byte>> decodeJpeg(std::span<const std::byte> input,
                                          std::uint32_t width,
                                          std::uint32_t height,
                                          std::uint32_t samplesPerPixel) {
    if (samplesPerPixel != 1 && samplesPerPixel != 3) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "decodeJpeg: samplesPerPixel must be 1 or 3"});
    }

    jpeg_decompress_struct cinfo{};
    JpegErrorContext errorContext{};
    cinfo.err = jpeg_std_error(&errorContext.pub);
    errorContext.pub.error_exit = jpegErrorExit;

    // Same non-trivial-destructor constraint as encodeJpeg: the output vector is only built
    // after jpeg_finish_decompress succeeds. rawOutput is a raw malloc'd buffer (trivial type)
    // so it is safe to hold across the setjmp/longjmp boundary.
    unsigned char* volatile rawOutput = nullptr;
    if (setjmp(errorContext.setjmpBuffer)) {
        std::free(rawOutput);
        jpeg_destroy_decompress(&cinfo);
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "decodeJpeg: libjpeg-turbo decompression failed"});
    }

    jpeg_create_decompress(&cinfo);
    jpeg_mem_src(&cinfo,
                 reinterpret_cast<const unsigned char*>(input.data()),
                 static_cast<unsigned long>(input.size()));
    jpeg_read_header(&cinfo, TRUE);

    // Reject a mismatched image size on the stream's own header, before jpeg_start_decompress
    // allocates buffers sized off it -- a hostile stream can declare far larger dimensions than
    // its (small) strip byte count would suggest, and jpeg_start_decompress on a progressive
    // JPEG allocates a whole-image coefficient array up front.
    if (cinfo.image_width != width || cinfo.image_height != height) {
        jpeg_destroy_decompress(&cinfo);
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeJpeg: JPEG stream dimensions do not match expected width/height"});
    }

    cinfo.out_color_space = samplesPerPixel == 1 ? JCS_GRAYSCALE : JCS_RGB;
    jpeg_start_decompress(&cinfo);

    if (static_cast<std::uint32_t>(cinfo.output_components) != samplesPerPixel) {
        jpeg_abort_decompress(&cinfo);
        jpeg_destroy_decompress(&cinfo);
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeJpeg: decoded component count does not match expected samplesPerPixel"});
    }

    const std::size_t rowStride = static_cast<std::size_t>(width) * samplesPerPixel;
    const std::size_t totalSize = rowStride * height;
    rawOutput = static_cast<unsigned char*>(std::malloc(totalSize));
    if (rawOutput == nullptr) {
        jpeg_destroy_decompress(&cinfo);
        return std::unexpected(Error{ErrorCode::InvalidArgument, "decodeJpeg: allocation failed"});
    }
    while (cinfo.output_scanline < cinfo.output_height) {
        JSAMPROW rowPointer[1] = {rawOutput +
                                  static_cast<std::size_t>(cinfo.output_scanline) * rowStride};
        if (jpeg_read_scanlines(&cinfo, rowPointer, 1) != 1) {
            std::free(rawOutput);
            jpeg_destroy_decompress(&cinfo);
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "decodeJpeg: libjpeg-turbo failed to read scanline"});
        }
    }
    jpeg_finish_decompress(&cinfo);
    jpeg_destroy_decompress(&cinfo);

    std::vector<std::byte> output(reinterpret_cast<std::byte*>(rawOutput),
                                  reinterpret_cast<std::byte*>(rawOutput) + totalSize);
    std::free(rawOutput);
    return output;
}

} // namespace ptiff::compression
