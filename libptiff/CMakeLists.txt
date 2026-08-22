add_library(ptiff)
add_library(ptiff::ptiff ALIAS ptiff)

include(GenerateExportHeader)
include(GitVersion)
include(CompilerWarnings)
include(Sanitizers)
include(PkgConfigPkg)

ptiff_git_version(PTIFF_GIT_HASH)

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/include/ptiff/core/version.hpp.in"
    "${CMAKE_CURRENT_BINARY_DIR}/include/ptiff/core/version.hpp"
    @ONLY
)

target_sources(ptiff
    PRIVATE
        src/core/version.cpp
        src/logging/logger.cpp
        src/logging/detail/spdlog_logger.cpp
        src/io/file_binary_reader.cpp
        src/io/file_binary_writer.cpp
        src/io/memory_binary_reader.cpp
        src/io/memory_binary_writer.cpp
        src/io/http_range_binary_reader.cpp
        src/io/reader.cpp
        src/io/writer.cpp
        src/io/storage_model.cpp
        src/io/backend_factory.cpp
        src/io/scene_deserializer.cpp
        src/io/scene_serializer.cpp
        src/io/backend/tiff_backend.cpp
        src/io/backend/tiff/tiff_header.cpp
        src/io/backend/tiff/tiff_header_writer.cpp
        src/io/backend/tiff/tiff_ifd.cpp
        src/io/backend/tiff/tiff_ifd_writer.cpp
        src/io/backend/tiff/tiff_pixel_format.cpp
        src/io/backend/tiff/tiff_directory.cpp
        src/io/backend/tiff/tiff_directory_writer.cpp
        src/io/backend/tiff/ptiff_metadata.cpp
        src/io/backend/tiff/tiff_image_source.cpp
        src/io/backend/tiff/tiff_image_sink.cpp
        # Compression codecs
        src/compression/packbits.cpp
        src/compression/lzw.cpp
        src/compression/predictor.cpp
        src/compression/deflate.cpp
        src/compression/jpeg.cpp
        src/io/backend/memory_backend.cpp
        src/io/backend/memory/memory_image_source.cpp
        src/io/backend/memory/memory_image_sink.cpp
        src/io/backend/pds4_backend.cpp
        src/io/backend/pds4/pds4_label.cpp
        src/io/backend/pds4/pds4_document.cpp
        src/io/backend/isis_backend.cpp
        src/io/backend/isis/isis_label.cpp
        src/io/backend/isis/isis_document.cpp
        src/io/backend/zarr_backend.cpp
        src/io/backend/zarr/zarr_document.cpp
        src/io/backend/zarr/zarr_image_source.cpp
        src/io/backend/zarr/zarr_image_sink.cpp
        src/io/backend/openexr_backend.cpp
        src/io/backend/openexr/openexr_stream.cpp
        src/io/backend/openexr/openexr_document.cpp
        src/io/backend/openexr/openexr_image_source.cpp
        src/io/backend/openexr/openexr_image_sink.cpp
        src/image.cpp
        src/scene.cpp
        src/metadata/metadata.cpp
        src/metadata/scientific_layer.cpp
        src/geometry/camera.cpp
        src/geometry/coordinate_reference_system.cpp
        src/geometry/lens_model.cpp
        src/geometry/projection.cpp
)

target_sources(ptiff
    PUBLIC
        FILE_SET HEADERS
        BASE_DIRS
            "${CMAKE_CURRENT_SOURCE_DIR}/include"
            "${CMAKE_CURRENT_BINARY_DIR}/include"
        FILES
            include/ptiff/ptiff.hpp
            include/ptiff/image.hpp
            include/ptiff/scene.hpp
            include/ptiff/core/error.hpp
            include/ptiff/core/id.hpp
            include/ptiff/core/precondition.hpp
            include/ptiff/core/result.hpp
            "${CMAKE_CURRENT_BINARY_DIR}/include/ptiff/core/version.hpp"
            include/ptiff/logging/logger.hpp
            include/ptiff/io/binary_reader.hpp
            include/ptiff/io/file_binary_reader.hpp
            include/ptiff/io/file_binary_writer.hpp
            include/ptiff/io/binary_writer.hpp
            include/ptiff/io/memory_binary_reader.hpp
            include/ptiff/io/memory_binary_writer.hpp
            include/ptiff/io/http_range_binary_reader.hpp
            include/ptiff/io/deserializer.hpp
            include/ptiff/io/reader.hpp
            include/ptiff/io/scene_deserializer.hpp
            include/ptiff/io/scene_serializer.hpp
            include/ptiff/io/serializer.hpp
            include/ptiff/io/storage_model.hpp
            include/ptiff/io/writer.hpp
            include/ptiff/io/tile/tile_extent.hpp
            include/ptiff/io/tile/tile_index.hpp
            include/ptiff/io/tile/tile_region.hpp
            include/ptiff/io/tile/tile_layout.hpp
            include/ptiff/io/tile/tile.hpp
            include/ptiff/io/tile/tile_cache.hpp
            include/ptiff/io/tile/tile_storage.hpp
            include/ptiff/io/tile/tile_iterator.hpp
            include/ptiff/io/backend_capabilities.hpp
            include/ptiff/io/image_source.hpp
            include/ptiff/io/image_sink.hpp
            include/ptiff/io/tile_provider.hpp
            include/ptiff/io/storage_backend.hpp
            include/ptiff/io/backend_factory.hpp
            include/ptiff/io/backend/tiff_backend.hpp
            include/ptiff/io/backend/memory_backend.hpp
            include/ptiff/io/backend/pds4_backend.hpp
            include/ptiff/io/backend/isis_backend.hpp
            include/ptiff/io/backend/zarr_backend.hpp
            include/ptiff/io/backend/openexr_backend.hpp
            include/ptiff/metadata/metadata.hpp
            include/ptiff/metadata/scientific_layer.hpp
            include/ptiff/geometry/camera.hpp
            include/ptiff/geometry/coordinate_reference_system.hpp
)

generate_export_header(ptiff
    BASE_NAME ptiff
    EXPORT_FILE_NAME "${CMAKE_CURRENT_BINARY_DIR}/include/ptiff/export.hpp"
)

target_sources(ptiff
    PUBLIC
        FILE_SET HEADERS
        BASE_DIRS "${CMAKE_CURRENT_BINARY_DIR}/include"
        FILES "${CMAKE_CURRENT_BINARY_DIR}/include/ptiff/export.hpp"
)

target_compile_features(ptiff PUBLIC cxx_std_23)

# All headers live under include/ (reachable via the FILE_SET HEADERS base dir registered
# on the ptiff target below); no header remains under src/, so no extra include path is
# needed here.
target_include_directories(ptiff
    PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/include
)

set_target_properties(ptiff PROPERTIES
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN ON
    VERSION ${PROJECT_VERSION}
    SOVERSION ${PROJECT_VERSION_MAJOR}
)

find_package(fmt REQUIRED CONFIG)
find_package(spdlog REQUIRED CONFIG)

# Milestone 3 backend dependencies (see ../conanfile.txt). Each is found here so the
# configuration provably resolves against the Conan toolchain; linking happens per backend
# phase (PDS4 -> pugixml, Zarr -> nlohmann_json + zstd/zlib, OpenEXR -> openexr) and is
# smoke-tested in libptiff/tests/unit/io/backend/deps/dependencies_smoke_test.cpp.
find_package(pugixml REQUIRED CONFIG)
find_package(nlohmann_json REQUIRED CONFIG)
find_package(OpenEXR REQUIRED CONFIG)
find_package(zstd REQUIRED CONFIG)
find_package(ZLIB REQUIRED)
find_package(libdeflate REQUIRED CONFIG)
find_package(libjpeg-turbo REQUIRED CONFIG)
find_package(CURL REQUIRED CONFIG)

target_link_libraries(ptiff
    PRIVATE
        fmt::fmt
        spdlog::spdlog
        # Milestone 3: the PDS4 backend reads/writes XML labels through pugixml; the Zarr backend
        # builds JSON array headers (nlohmann_json) and compresses chunks via zstd/zlib; the
        # OpenEXR backend reads/writes real .exr documents through the OpenEXR C++ API.
        pugixml::pugixml
        nlohmann_json::nlohmann_json
        openexr::openexr
        zstd::libzstd_static
        ZLIB::ZLIB
        libjpeg-turbo::jpeg-static
        CURL::libcurl
        $<BUILD_INTERFACE:ptiff_warnings>
)

ptiff_enable_sanitizers(ptiff)

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

install(TARGETS ptiff
    EXPORT ptiffTargets
    FILE_SET HEADERS DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

install(EXPORT ptiffTargets
    FILE ptiffTargets.cmake
    NAMESPACE ptiff::
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff
)

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/ptiffConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/ptiffConfig.cmake"
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff
)

write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/ptiffConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/ptiffConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/ptiffConfigVersion.cmake"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff
)

# pkg-config (for Rust/Python/Ruby consumers that do not use CMake).
ptiff_pkg_config(libptiff "${CMAKE_CURRENT_SOURCE_DIR}/cmake/libptiff.pc.in")

if(PTIFF_BUILD_TESTS)
    add_subdirectory(tests)
endif()

if(PTIFF_BUILD_EXAMPLES)
    add_subdirectory(examples)
endif()

if(PTIFF_BUILD_BENCHMARKS)
    add_subdirectory(benchmarks)
endif()

# C ABI binding (lang-agnostic libptiff_c). Added from here so the CMake scope
# can see the `ptiff` target created above; it is the shared substrate that the
# Go (and future Rust/Python/Ruby) bindings build on.
if(PTIFF_BUILD_C_BINDINGS)
    add_subdirectory("${CMAKE_SOURCE_DIR}/bindings/c"
                     "${CMAKE_BINARY_DIR}/bindings/c")
endif()

# Go bindings. Added from here (rather than the repo root) so the CMake scope
# can see the `ptiff`, `fmt::fmt`, `spdlog::spdlog` and (implicitly) the
# `ptiff_c` targets created above.
if(PTIFF_BUILD_GO_BINDINGS)
    add_subdirectory("${CMAKE_SOURCE_DIR}/bindings/go"
                     "${CMAKE_BINARY_DIR}/bindings/go")
endif()
