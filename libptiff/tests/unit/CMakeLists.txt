add_executable(ptiff_unit_tests
    api_surface/traits_test.cpp
    core/error_test.cpp
    core/result_test.cpp
    core/scene_test.cpp
    core/version_test.cpp
    logging/logger_test.cpp
    io/backend_factory_test.cpp
    io/backends_test.cpp
    io/binary_io_test.cpp
    io/file_binary_reader_test.cpp
    io/file_binary_writer_test.cpp
    io/image_source_sink_test.cpp
    io/reader_writer_test.cpp
    io/read_pixels_test.cpp
    io/write_pixels_test.cpp
    io/scene_deserializer_test.cpp
    io/scene_serializer_test.cpp
    io/scene_storage_roundtrip_test.cpp
    io/serializer_test.cpp
    io/storage_backend_test.cpp
    io/storage_model_test.cpp
    io/memory_binary_test.cpp
    io/http_range_binary_reader_test.cpp
    io/tile_interfaces_test.cpp
    io/tile_layout_test.cpp
    io/tile_test.cpp
    io/tile_value_types_test.cpp
    # TIFF backend
    io/backend/tiff/tiff_endian_test.cpp
    io/backend/tiff/tiff_header_test.cpp
    io/backend/tiff/tiff_header_writer_test.cpp
    io/backend/tiff/tiff_ifd_test.cpp
    io/backend/tiff/tiff_checked_arithmetic_test.cpp
    io/backend/tiff/tiff_parser_hardening_test.cpp
    io/backend/tiff/tiff_ifd_writer_test.cpp
    io/backend/tiff/tiff_pixel_format_test.cpp
    io/backend/tiff/tiff_directory_test.cpp
    io/backend/tiff/tiff_directory_writer_test.cpp
    io/backend/tiff/ptiff_metadata_test.cpp
    io/backend/tiff/tiff_ptiff_metadata_roundtrip_test.cpp
    io/backend/tiff/tiff_image_source_test.cpp
    io/backend/tiff/tiff_image_sink_test.cpp
    io/backend/tiff/tiff_backend_e2e_test.cpp
    io/backend/tiff/policy/tiff_policy_test.cpp
    # Memory backend
    io/backend/memory/memory_codec_test.cpp
    io/backend/memory/memory_backend_test.cpp
    io/backend/memory/memory_pixels_test.cpp
    # Milestone 3 dependencies smoke test (each conan lib linked + exercised)
    io/backend/deps/dependencies_smoke_test.cpp
    # PDS4 backend
    io/backend/pds4/pds4_backend_test.cpp
    # ISIS3 backend
    io/backend/isis/isis_backend_test.cpp
    # Zarr backend
    io/backend/zarr/zarr_backend_test.cpp
    # OpenEXR backend
    io/backend/openexr/openexr_backend_test.cpp
    # Compression codecs
    compression/packbits_test.cpp
    compression/lzw_test.cpp
    compression/predictor_test.cpp
    compression/deflate_test.cpp
    compression/jpeg_test.cpp
)

# Internal (non-installed) headers under include/ptiff/ are reached from the tests via the
# <ptiff/...> include style; point at include/ (previously the tests reached the same internal
# headers through target_include_directories on src/).
target_include_directories(ptiff_unit_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../include)

# Backend stubs (libptiff/src/io/backend/*.cpp) self-register with BackendFactory via a static
# initializer with no externally visible symbol. A plain static-library link would let the linker
# discard those object files as unreferenced; WHOLE_ARCHIVE forces every object file in ptiff to be
# linked in so the registrations actually run.
target_link_libraries(ptiff_unit_tests
    PRIVATE
        $<LINK_LIBRARY:WHOLE_ARCHIVE,ptiff::ptiff>
        Catch2::Catch2WithMain
        ptiff_warnings
        # Milestone 3 backend dependencies (see ../conanfile.txt). Each is exercised in
        # dependencies_smoke_test.cpp; linking them here proves the wiring end-to-end.
        pugixml::pugixml
        nlohmann_json::nlohmann_json
        openexr::openexr
        zstd::libzstd_static
        ZLIB::ZLIB
        libdeflate::libdeflate_static
        libjpeg-turbo::jpeg-static
        httplib::httplib
)

catch_discover_tests(ptiff_unit_tests)
