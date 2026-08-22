# Standalone C ABI binding for libptiff (bindings/c).
#
# Builds libptiff_c, a language-agnostic "extern C" veneer over the implemented
# core of libptiff's public C++ API. Because every foreign-language binding
# (Go cgo, Rust FFI, Python ctypes/cffi, Ruby FFI, ...) can talk to a plain C
# ABI, this library is the single source of truth for all of them: versioning,
# errors, logging, the backend registry, and the Image domain type.
#
# Guarded by PTIFF_BUILD_C_BINDINGS. Works with either shared or static libptiff.
#
# The generated libptiff_c is meant to be consumed by binding projects via the
# ptiff_c::ptiff_c CMake target (or by linking -lptiff_c with the bridge headers
# from ${CMAKE_CURRENT_SOURCE_DIR}).

include(PkgConfigPkg)

set(_ptiff_c_sources
    ptiff_bridge.cpp
    ptiff_image_bridge.cpp
    ptiff_pixel_bridge.cpp
    ptiff_metadata.cpp
    ptiff_version.cpp
    ptiff_logger.cpp
)

add_library(ptiff_c ${_ptiff_c_sources})
add_library(ptiff_c::ptiff_c ALIAS ptiff_c)

target_link_libraries(ptiff_c PRIVATE ptiff)

# The bridge headers are public: foreign bindings (and downstream C consumers)
# include them directly.
target_include_directories(ptiff_c PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
    $<INSTALL_INTERFACE:include>
)

# Ensure the point-of-definition C++ standard matches what libptiff requires.
if(CMAKE_CXX_STANDARD LESS 23)
    set_target_properties(ptiff_c PROPERTIES
        CXX_STANDARD 23
        CXX_STANDARD_REQUIRED ON
    )
endif()

set_target_properties(ptiff_c PROPERTIES
    OUTPUT_NAME ptiff_c
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN YES
    POSITION_INDEPENDENT_CODE ON
)

if(BUILD_SHARED_LIBS)
    # A shared libptiff_c keeps backend self-registration visible and hides the
    # C++ runtime/deps behind the C surface.
    set_target_properties(ptiff_c PROPERTIES
        CXX_HIDDEN_VISIBILITY ON
    )
    # Embed an install rpath so the installed libptiff_c.dylib can find its
    # transitive libptiff, which sits in the same install lib/ dir, without any
    # DYLD_-/LD_ environment plumbing. This is what makes ctypes/Fiddle-based
    # bindings (Python/Ruby) load cleanly from an install prefix.
    if(APPLE)
        set_property(TARGET ptiff_c PROPERTY INSTALL_RPATH "@loader_path")
    elseif(UNIX)
        set_property(TARGET ptiff_c PROPERTY INSTALL_RPATH "$ORIGIN")
    endif()
endif()

# Install rules (only meaningful downstream; harmless when building in-tree).
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
install(TARGETS ptiff_c
    EXPORT ptiff_cTargets
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)
# All three headers are public: the two bridge headers, plus the export macro
# header they include.
install(FILES
    ptiff_bridge.h
    ptiff_image_bridge.h
    ptiff_pixel_bridge.h
    ptiff_metadata.h
    ptiff_c_export.h
    ptiff_version.h
    ptiff_logger.h
    ptiff_error.h
    ptiff_camera.h
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
)
install(EXPORT ptiff_cTargets
    FILE ptiff_cTargets.cmake
    NAMESPACE ptiff_c::
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff_c
)
# A tiny config so downstream can find_package(ptiff_c CONFIG).
configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/ptiff_cConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/ptiff_cConfig.cmake"
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff_c
)
install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/ptiff_cConfig.cmake"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ptiff_c
)

# pkg-config (for Rust/Python/Ruby consumers that do not use CMake). libptiff_c
# requires the installed libptiff .pc, so downstream gets the transitive -lptiff
# and header dirs too.
ptiff_pkg_config(libptiff_c "${CMAKE_CURRENT_SOURCE_DIR}/libptiff_c.pc.in")

# Tests for this C ABI (exercised straight through the plain C functions, the same surface every
# language binding calls), guarded the same way libptiff/tests is.
if(PTIFF_BUILD_TESTS)
    find_package(Catch2 3 REQUIRED)

    add_executable(ptiff_c_bindings_tests
        tests/ptiff_pixel_bridge_test.cpp
    )
    target_link_libraries(ptiff_c_bindings_tests
        PRIVATE
            ptiff_c::ptiff_c
            ptiff::ptiff
            Catch2::Catch2WithMain
    )

    include(CTest)
    include(Catch)
    catch_discover_tests(ptiff_c_bindings_tests)
endif()
