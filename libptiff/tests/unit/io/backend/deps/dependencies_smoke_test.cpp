// Dependencies smoke test for Milestone 3 backends.
//
// Proves that every third-party library added in ../conanfile.txt resolves, links and runs
// against the ptiff gnu23/Debug toolchain. Each section exercises the minimal real API of one
// library so a break in the dependency wiring (wrong target name, missing transitives, ABI
// mismatch) fails here -- in isolation from any backend code.

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <string>
#include <vector>

#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>

// pugixml (PDS4 XML labels)
#include <pugixml.hpp>
// nlohmann_json (Zarr .zarray/.zattrs)
#include <nlohmann/json.hpp>
// OpenEXR (OpenEXR backend)
#include <ImfHeader.h>
#include <ImfRgbaFile.h>

// zstd / zlib / libdeflate (compression)
extern "C" {
#include <zlib.h>
#include <zstd.h>
}
#include <libdeflate.h>
// libjpeg-turbo (TIFF Compression=7)
extern "C" {
#include <jpeglib.h>
}

// libcurl (cloud HTTP-range transport)
#include <curl/curl.h>

namespace {

// ---------------------------------------------------------------------------
// pugixml
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: pugixml loads an XML label", "[deps-smoke]") {
    const char* label = "<Product_Observational><imageWidth>2</imageWidth>"
                        "<imageHeight>3</imageHeight></Product_Observational>";
    pugi::xml_document doc;
    const auto result = doc.load_string(label);
    REQUIRE(result);
    const auto prod = doc.child("Product_Observational");
    REQUIRE(prod);
    REQUIRE(std::string{prod.child("imageWidth").child_value()} == "2");
    REQUIRE(std::string{prod.child("imageHeight").child_value()} == "3");
}

// ---------------------------------------------------------------------------
// nlohmann_json
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: nlohmann_json parses .zarray", "[deps-smoke]") {
    const auto json =
        nlohmann::json::parse(R"({"shape":[2,2],"chunks":[2,2],"dtype":"|u1","compressor":null})");
    REQUIRE(json["shape"].size() == 2);
    REQUIRE(json["shape"][0].get<int>() == 2);
    REQUIRE(json["shape"][1].get<int>() == 2);
    REQUIRE(std::string{json["dtype"]} == "|u1");
}

// ---------------------------------------------------------------------------
// OpenEXR (write + read a trivial 1x1 RGBA image through Imf::RgbaFile API)
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: OpenEXR writes and reads a 1x1 Rgba file", "[deps-smoke]") {
    const auto path = (std::filesystem::temp_directory_path() / "ptiff_deps_smoke.exr").string();

    try {
        {
            // Writing scope: RgbaOutputFile flushes/finalizes on destruction (no close()), so
            // it must go out of scope before we re-open the file for reading.
            Imf::Header header(1, 1); // one line, one column
            header.compression() = Imf::NO_COMPRESSION;
            Imf::RgbaOutputFile out(path.c_str(), header, Imf::WRITE_RGBA);
            const Imf::Rgba pixel{1.0f, 0.5f, 0.25f, 1.0f};
            out.setFrameBuffer(&pixel, 1, 0);
            out.writePixels(1);
        }

        Imf::RgbaInputFile in(path.c_str());
        const auto dw = in.header().dataWindow();
        REQUIRE(dw.max.x - dw.min.x + 1 == 1); // one column
        REQUIRE(dw.max.y - dw.min.y + 1 == 1); // one line
        Imf::Rgba back{0.0f, 0.0f, 0.0f, 0.0f};
        in.setFrameBuffer(&back, 1, 0);
        in.readPixels(0);
        REQUIRE(back.r == Catch::Approx(1.0f));
        REQUIRE(back.g == Catch::Approx(0.5f));
    } catch (const std::exception& e) {
        FAIL("OpenEXR threw: " << e.what());
    }

    std::filesystem::remove(path);
}

// ---------------------------------------------------------------------------
// zstd
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: zstd compress/decompress round-trips", "[deps-smoke]") {
    const std::string data =
        "ptiff zstd smoke payload, repeated. ptiff zstd smoke payload, repeated.";
    std::vector<std::byte> compressed(ZSTD_compressBound(data.size()));
    const std::size_t compressedSize =
        ZSTD_compress(compressed.data(), compressed.size(), data.data(), data.size(), 3);
    REQUIRE(!ZSTD_isError(compressedSize));

    std::string decompressed(data.size(), '\0');
    const std::size_t decoded = ZSTD_decompress(
        decompressed.data(), decompressed.size(), compressed.data(), compressedSize);
    REQUIRE(!ZSTD_isError(decoded));
    REQUIRE(decoded == data.size());
    REQUIRE(decompressed == data);
}

// ---------------------------------------------------------------------------
// zlib
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: zlib deflate/inflate round-trips", "[deps-smoke]") {
    const std::string data = "ptiff zlib smoke payload, repeated repeatedly. ";
    // zlib's API uses Bytef* (= unsigned char*); std::vector<std::byte> would need casts on
    // every argument, so use unsigned char locally.
    std::vector<unsigned char> compressed(compressBound(static_cast<uLong>(data.size())));
    uLongf compressedSize = static_cast<uLongf>(compressed.size());
    const int rc = compress2(compressed.data(),
                             &compressedSize,
                             reinterpret_cast<const Bytef*>(data.data()),
                             static_cast<uLong>(data.size()),
                             6);
    REQUIRE(rc == Z_OK);

    std::string decompressed(data.size(), '\0');
    uLongf decodedSize = static_cast<uLongf>(decompressed.size());
    const int urc = uncompress(reinterpret_cast<Bytef*>(decompressed.data()),
                               &decodedSize,
                               compressed.data(),
                               compressedSize);
    REQUIRE(urc == Z_OK);
    REQUIRE(decodedSize == data.size());
    REQUIRE(decompressed == data);
}

// ---------------------------------------------------------------------------
// libdeflate
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: libdeflate com/exde-compresses", "[deps-smoke]") {
    const std::string data = "ptiff libdeflate smoke payload. ptiff libdeflate smoke payload.";

    // Use libdeflate's zlib wrapper (libdeflate_zlib_compress / _decompress) which carries a
    // standard zlib header, so the round-trip is unambiguous and checks both directions.
    auto* compressor = libdeflate_alloc_compressor(6);
    REQUIRE(compressor != nullptr);

    // zlib wrapper carries a standard zlib header; its bound equals the deflate bound.
    const std::size_t bound = libdeflate_deflate_compress_bound(compressor, data.size());
    std::vector<std::byte> compressed(bound);

    const std::size_t compressedSize = libdeflate_zlib_compress(
        compressor, data.data(), data.size(), compressed.data(), compressed.size());
    REQUIRE(compressedSize > 0);

    auto* decompressor = libdeflate_alloc_decompressor();
    REQUIRE(decompressor != nullptr);
    std::string decompressed(data.size(), '\0');
    std::size_t actualLen = 0;
    const libdeflate_result rc = libdeflate_zlib_decompress(decompressor,
                                                            compressed.data(),
                                                            compressedSize,
                                                            decompressed.data(),
                                                            decompressed.size(),
                                                            &actualLen);
    REQUIRE(rc == LIBDEFLATE_SUCCESS);
    REQUIRE(actualLen == data.size());
    REQUIRE(decompressed == data);

    libdeflate_free_compressor(compressor);
    libdeflate_free_decompressor(decompressor);
}

// ---------------------------------------------------------------------------
// libjpeg-turbo
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: libjpeg-turbo compresses and decompresses a 4x4 grayscale image",
          "[deps-smoke]") {
    constexpr int kWidth = 4;
    constexpr int kHeight = 4;
    std::vector<unsigned char> pixels(static_cast<std::size_t>(kWidth) * kHeight);
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        pixels[i] = static_cast<unsigned char>(i * 16);
    }

    jpeg_compress_struct cinfo{};
    jpeg_error_mgr jerr{};
    cinfo.err = jpeg_std_error(&jerr);
    jpeg_create_compress(&cinfo);

    unsigned char* outBuffer = nullptr;
    unsigned long outSize = 0;
    jpeg_mem_dest(&cinfo, &outBuffer, &outSize);

    cinfo.image_width = kWidth;
    cinfo.image_height = kHeight;
    cinfo.input_components = 1;
    cinfo.in_color_space = JCS_GRAYSCALE;
    jpeg_set_defaults(&cinfo);
    jpeg_set_quality(&cinfo, 90, TRUE);
    jpeg_start_compress(&cinfo, TRUE);
    while (cinfo.next_scanline < cinfo.image_height) {
        JSAMPROW rowPointer[1];
        rowPointer[0] = &pixels[static_cast<std::size_t>(cinfo.next_scanline) * kWidth];
        jpeg_write_scanlines(&cinfo, rowPointer, 1);
    }
    jpeg_finish_compress(&cinfo);
    jpeg_destroy_compress(&cinfo);

    REQUIRE(outSize > 0);

    jpeg_decompress_struct dinfo{};
    jpeg_error_mgr djerr{};
    dinfo.err = jpeg_std_error(&djerr);
    jpeg_create_decompress(&dinfo);
    jpeg_mem_src(&dinfo, outBuffer, outSize);
    jpeg_read_header(&dinfo, TRUE);
    jpeg_start_decompress(&dinfo);
    REQUIRE(dinfo.output_width == kWidth);
    REQUIRE(dinfo.output_height == kHeight);
    REQUIRE(dinfo.output_components == 1);

    std::vector<unsigned char> decoded(pixels.size());
    while (dinfo.output_scanline < dinfo.output_height) {
        JSAMPROW rowPointer[1];
        rowPointer[0] = &decoded[static_cast<std::size_t>(dinfo.output_scanline) * kWidth];
        jpeg_read_scanlines(&dinfo, rowPointer, 1);
    }
    jpeg_finish_decompress(&dinfo);
    jpeg_destroy_decompress(&dinfo);
    free(outBuffer);

    // Lossy codec -- tolerance compare, not byte-exact.
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(std::abs(static_cast<int>(decoded[i]) - static_cast<int>(pixels[i])) <= 15);
    }
}

// ---------------------------------------------------------------------------
// libcurl
// ---------------------------------------------------------------------------
TEST_CASE("dependency smoke: libcurl initializes and reports its version", "[deps-smoke]") {
    CURLcode initResult = curl_global_init(CURL_GLOBAL_DEFAULT);
    REQUIRE(initResult == CURLE_OK);

    CURL* handle = curl_easy_init();
    REQUIRE(handle != nullptr);
    curl_easy_cleanup(handle);

    const curl_version_info_data* info = curl_version_info(CURLVERSION_NOW);
    REQUIRE(info != nullptr);
    REQUIRE(info->version != nullptr);

    curl_global_cleanup();
}

} // namespace
