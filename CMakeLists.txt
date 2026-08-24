# PTIFF — Rust-first packaging orchestrator
#
# PTIFF 1.0 is implemented entirely in Rust (ptiff-core -> ptiff-rust ->
# ptiff-cli / ptiff-c). There is no C++ library any more, so this CMakeLists no
# longer builds `libptiff` from libptiff/ (that directory is removed). Its
# remaining jobs are:
#
#   * building the C ABI (`libptiff_c`, crates/ptiff-c) and the Rust CLI
#     (`ptiff`, crates/ptiff-cli) with Cargo,
#   * installing them (bin, lib, header, pkg-config) so the SWIG/Go/Python/Ruby/
#     Octave bindings and end users can consume them,
#   * generating native packages (CPack deb/rpm on Linux, productbuild .pkg on
#     macOS) around those Rust artifacts,
#   * building the project docs with Doxygen.
#
# `cargo build` is the real build; this file is a thin wrapper for packaging.

cmake_minimum_required(VERSION 3.26)

project(
    PTIFF
    VERSION 1.0.0
    LANGUAGES NONE
)

option(PTIFF_BUILD_C_ABI "Build the C ABI libptiff_c from the Rust crate (crates/ptiff-c)" OFF)
option(PTIFF_BUILD_CLI "Build and install the Rust ptiff CLI (crates/ptiff-cli)" OFF)
option(PTIFF_BUILD_GO_BINDINGS "Build and test the cgo Go bindings (bindings/go)" OFF)
option(PTIFF_BUILD_DOCS "Build the Doxygen API documentation" ON)

# The Go binding needs the C ABI library, so enabling it requires building it.
if(PTIFF_BUILD_GO_BINDINGS AND NOT PTIFF_BUILD_C_ABI)
    set(PTIFF_BUILD_C_ABI ON CACHE BOOL "Build the C ABI libptiff_c from the Rust crate (crates/ptiff-c)" FORCE)
endif()

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type" FORCE)
endif()

include(GNUInstallDirs)

# The Cargo equivalent of the CMake build type is the lowercase profile name
# ("Release" -> "release", "Debug" -> "debug"); Cargo has exactly these two.
if(CMAKE_BUILD_TYPE MATCHES "[Dd]ebug")
    set(PTIFF_CARGO_PROFILE "debug")
    set(PTIFF_CARGO_FLAG "--debug")
else()
    set(PTIFF_CARGO_PROFILE "release")
    set(PTIFF_CARGO_FLAG "--release")
endif()

set(PTIFF_CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo-target")

if(PTIFF_BUILD_GO_BINDINGS)
    enable_testing()
endif()

# ---------------------------------------------------------------------------
# Rust C ABI (libptiff_c) + CLI (ptiff) -- built with Cargo
# ---------------------------------------------------------------------------
# The C ABI is emitted to <build>/cargo-target/<profile>/ and the header to
# <build>/cargo-target/ptiff_c.h (stable path from crates/ptiff-c/build.rs).

set(PTIFF_C_LIB_DIR "${PTIFF_CARGO_TARGET_DIR}/${PTIFF_CARGO_PROFILE}")

if(PTIFF_BUILD_C_ABI)
    message(STATUS "PTIFF_BUILD_C_ABI: building C ABI libptiff_c (cargo build -p ptiff-c)")
    execute_process(
        COMMAND ${CMAKE_COMMAND} -E env
                "CARGO_TARGET_DIR=${PTIFF_CARGO_TARGET_DIR}"
                cargo build -p ptiff-c ${PTIFF_CARGO_FLAG}
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        RESULT_VARIABLE _ptiff_c_cargo_result
    )
    if(NOT _ptiff_c_cargo_result EQUAL 0)
        message(FATAL_ERROR "cargo build -p ptiff-c failed")
    endif()

    # Shared library name is platform-dependent.
    if(WIN32)
        set(PTIFF_C_SHARED "ptiff_c.dll")
        set(PTIFF_C_SHARED_IMPLIB "ptiff_c.dll.lib")
    elseif(APPLE)
        set(PTIFF_C_SHARED "libptiff_c.dylib")
    else()
        set(PTIFF_C_SHARED "libptiff_c.so")
    endif()
    set(PTIFF_C_STATIC "libptiff_c.a")

    install(PROGRAMS "${PTIFF_C_LIB_DIR}/${PTIFF_C_SHARED}"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}")
    install(FILES "${PTIFF_C_LIB_DIR}/${PTIFF_C_STATIC}"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}")

    # pkg-config for C/FFI consumers.
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/cmake/ptiff_c.pc.in"
        "${CMAKE_CURRENT_BINARY_DIR}/ptiff_c.pc"
        @ONLY)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/ptiff_c.pc"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")

    # Header (cbindgen output at the stable build path).
    set(PTIFF_C_HEADER "${PTIFF_CARGO_TARGET_DIR}/ptiff_c.h")
    install(FILES "${PTIFF_C_HEADER}"
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
endif()

if(PTIFF_BUILD_GO_BINDINGS)
    find_program(GO_EXECUTABLE go)
    find_program(SWIG_EXECUTABLE swig)
    find_program(MAKE_EXECUTABLE NAMES make gmake)
    if(GO_EXECUTABLE AND SWIG_EXECUTABLE AND MAKE_EXECUTABLE)
        set(_swig_dir "${CMAKE_CURRENT_SOURCE_DIR}/bindings/swig")
        add_test(NAME ptiff_go_bindings_generate
            COMMAND ${CMAKE_COMMAND} -E env
                    "PTIFF_C_LIB_DIR=${PTIFF_C_LIB_DIR}"
                    ${MAKE_EXECUTABLE} -C ${_swig_dir} go
        )
        add_test(NAME ptiff_go_bindings
            COMMAND ${GO_EXECUTABLE} test -count=1 ./ptiff
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/bindings/go"
        )
        set_tests_properties(ptiff_go_bindings PROPERTIES
            TIMEOUT 300
            DEPENDS ptiff_go_bindings_generate
        )
    else()
        message(WARNING "PTIFF_BUILD_GO_BINDINGS is ON but go/swig/make were not "
                        "all found; skipping Go binding build.")
    endif()
endif()

# ---------------------------------------------------------------------------
# Doxygen documentation
# ---------------------------------------------------------------------------
if(PTIFF_BUILD_DOCS)
    find_package(Doxygen QUIET)
    if(DOXYGEN_FOUND)
        add_custom_target(docs
            COMMAND ${CMAKE_COMMAND} -E echo "Generating Doxygen documentation ..."
            COMMAND ${DOXYGEN_EXECUTABLE} "${CMAKE_CURRENT_SOURCE_DIR}/Doxyfile"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
            COMMENT "Generating Doxygen documentation for PTIFF (html + xml)"
            VERBATIM)
    else()
        message(WARNING "PTIFF_BUILD_DOCS is ON but Doxygen was not found; the "
                        "'docs' target will not be generated.")
    endif()
endif()

# ---------------------------------------------------------------------------
# ptiff CLI (Rust) integration
# ---------------------------------------------------------------------------
if(PTIFF_BUILD_CLI)
    find_program(CARGO_EXECUTABLE cargo)
    if(CARGO_EXECUTABLE)
        set(PTIFF_CLI_BINARY "${PTIFF_CARGO_TARGET_DIR}/${PTIFF_CARGO_PROFILE}/ptiff")
        add_custom_target(ptiff_cli
            COMMAND ${CMAKE_COMMAND} -E env
                "CARGO_TARGET_DIR=${PTIFF_CARGO_TARGET_DIR}"
                ${CARGO_EXECUTABLE} build ${PTIFF_CARGO_FLAG}
                --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/crates/ptiff-cli/Cargo.toml"
            BYPRODUCTS "${PTIFF_CLI_BINARY}"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/crates/ptiff-cli"
            COMMENT "Building ptiff CLI (Rust, workspace crate crates/ptiff-cli)"
            VERBATIM)
        configure_file(
            "${CMAKE_CURRENT_SOURCE_DIR}/cmake/InstallPtiffCli.cmake.in"
            "${CMAKE_CURRENT_BINARY_DIR}/cmake/InstallPtiffCli.cmake"
            @ONLY)
        install(SCRIPT "${CMAKE_CURRENT_BINARY_DIR}/cmake/InstallPtiffCli.cmake")
    else()
        message(WARNING "PTIFF_BUILD_CLI=ON but cargo was not found; the ptiff CLI "
                        "will not be built/installed.")
    endif()
endif()

# ---------------------------------------------------------------------------
# Packaging (CPack)
# ---------------------------------------------------------------------------
set(CPACK_PACKAGE_NAME "ptiff")
set(CPACK_PACKAGE_VENDOR "PTIFF project")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Planetary TIFF (PTIFF) image format library (Rust core + C ABI + CLI)")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://afeldman.github.io/ptiff/")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
set(CPACK_PACKAGE_CONTACT "anton.feldmann@udo.edu")

include(InstallRequiredSystemLibraries)

if(UNIX AND NOT APPLE)
    # GNU/Linux: DEB and RPM. Files belong in the FHS system prefix.
    set(CPACK_PACKAGING_INSTALL_PREFIX "/usr")
    set(CPACK_GENERATOR "DEB;RPM")
    set(CPACK_DEBIAN_PACKAGE_MAINTAINER "PTIFF project")
    set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
    set(CPACK_DEBIAN_PACKAGE_DEPENDS "")
    set(CPACK_RPM_PACKAGE_LICENSE "Apache-2.0")
    set(CPACK_RPM_PACKAGE_GROUP "Applications")
    set(CPACK_PACKAGE_FILE_NAME "ptiff-${PROJECT_VERSION}-Linux")
elseif(APPLE)
    # macOS: a flat installer (.pkg) via the productbuild generator.
    set(CPACK_PACKAGING_INSTALL_PREFIX "/usr/local")
    set(CPACK_GENERATOR "productbuild")
    set(CPACK_PACKAGE_FILE_NAME "ptiff-${PROJECT_VERSION}-macOS")
endif()

set(CPACK_STRIP_FILES ON)
set(CPACK_SOURCE_IGNORE_FILES
    "build/;build-rel/;build-debug/;.git/;.gitignore;.github/;.nojekyll"
    "/crates/ptiff-cli/target")

include(CPack)
