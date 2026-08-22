cmake_minimum_required(VERSION 3.26)

project(
    PTIFF
    VERSION 0.4.2
    LANGUAGES CXX C
)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/libptiff/cmake")

option(PTIFF_BUILD_TESTS "Build libptiff unit/integration tests" ON)
option(PTIFF_BUILD_EXAMPLES "Build libptiff examples" OFF)
option(PTIFF_BUILD_BENCHMARKS "Build libptiff benchmarks" OFF)
option(PTIFF_ENABLE_ASAN "Enable AddressSanitizer" OFF)
option(PTIFF_ENABLE_UBSAN "Enable UndefinedBehaviorSanitizer" OFF)
option(PTIFF_ENABLE_TSAN "Enable ThreadSanitizer" OFF)
option(BUILD_SHARED_LIBS "Build libptiff as a shared library" OFF)
option(PTIFF_BUILD_GO_BINDINGS "Build and test the cgo Go bindings (bindings/go)" OFF)
option(PTIFF_BUILD_C_BINDINGS "Build the standalone C ABI binding libptiff_c (bindings/c)" OFF)
option(PTIFF_BUILD_CLI "Build and install the Rust ptiff CLI (ptiff-cli/)" OFF)
option(PTIFF_BUILD_INTEROP_SCRIPTS "Build the paper's interop/overhead fixture generator (scripts/)" OFF)

# The Go binding needs the C ABI library, so enabling it requires libptiff_c.
if(PTIFF_BUILD_GO_BINDINGS AND NOT PTIFF_BUILD_C_BINDINGS)
    set(PTIFF_BUILD_C_BINDINGS ON CACHE BOOL "Build the standalone C ABI binding libptiff_c (bindings/c)" FORCE)
endif()

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE "Debug" CACHE STRING "Build type" FORCE)
endif()

if(PTIFF_BUILD_TESTS OR PTIFF_BUILD_GO_BINDINGS)
    enable_testing()
endif()

add_subdirectory(libptiff)

if(PTIFF_BUILD_INTEROP_SCRIPTS)
    add_subdirectory(scripts)
endif()

if(PTIFF_BUILD_EXAMPLES)
    # Root-level end-user examples (real ESA/NASA TIFF readers).
    add_subdirectory(examples)
endif()

# ---------------------------------------------------------------------------
# Doxygen documentation
# ---------------------------------------------------------------------------
option(PTIFF_BUILD_DOCS "Build the Doxygen API documentation" ON)

if(PTIFF_BUILD_DOCS)
    find_package(Doxygen QUIET)

    if(DOXYGEN_FOUND)
        # Single source of truth for the docs configuration is the hand-written
        # Doxyfile in the repo root. Running `doxygen Doxyfile` directly and via
        # CMake therefore produces identical output. The Doxyfile writes its
        # output into build/docs/ and reads the latest Doxygen comments from
        # libptiff/include/, which is why rebuilding is a no-op-ish stamp target
        # below (re-running just regenerates; cheap).
        add_custom_target(docs
            COMMAND ${CMAKE_COMMAND} -E echo "Generating Doxygen documentation ..."
            COMMAND ${DOXYGEN_EXECUTABLE} "${CMAKE_CURRENT_SOURCE_DIR}/Doxyfile"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
            COMMENT "Generating Doxygen documentation for libptiff (html + xml)"
            VERBATIM)
    else()
        message(WARNING
            "Doxygen not found -- 'PTIFF_BUILD_DOCS' is ON but the 'docs' "
            "target will not be generated. Install doxygen "
            "(brew install doxygen) or set -DPTIFF_BUILD_DOCS=OFF.")
    endif()
endif()

# ---------------------------------------------------------------------------
# ptiff CLI (Rust) integration
# ---------------------------------------------------------------------------
# The CLI in ptiff-cli/ is a standalone Cargo project (`ptiff` binary) that
# links against an *installed* libptiff_c (via its relocatable pkg-config file,
# PTIFF_C_LIB_DIR is the in-tree fallback). CPack packages whatever the install
# rules describe, so the CLI binary must exist when `cpack` runs. It is a
# separate, non-`ALL` target: a packaging job builds the C/C++ libs first,
# installs them to a staging prefix, then builds `ptiff_cli` with
# PKG_CONFIG_PATH pointing at that prefix, and finally runs `cpack`.
if(PTIFF_BUILD_CLI)
    find_program(CARGO_EXECUTABLE cargo)
    if(CARGO_EXECUTABLE)
        set(PTIFF_CLI_BINARY "${CMAKE_CURRENT_BINARY_DIR}/ptiff-cli/target/release/ptiff")
        add_custom_target(ptiff_cli
            COMMAND ${CMAKE_COMMAND} -E env
                "PKG_CONFIG_PATH=$ENV{PKG_CONFIG_PATH}"
                "CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/ptiff-cli/target"
                ${CARGO_EXECUTABLE} build --release
                --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/ptiff-cli/Cargo.toml"
            BYPRODUCTS "${PTIFF_CLI_BINARY}"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/ptiff-cli"
            COMMENT "Building ptiff CLI (Rust, --release, pkg-config against install prefix)"
            VERBATIM)
        # Install the CLI only once its binary exists (see
        # cmake/InstallPtiffCli.cmake.in). A plain libs-first
        # `cmake --install --prefix <staging>` succeeds because the cargo binary
        # is built later; CPack re-installs after the CLI build and picks it up.
        configure_file(
            "${CMAKE_CURRENT_SOURCE_DIR}/cmake/InstallPtiffCli.cmake.in"
            "${CMAKE_CURRENT_BINARY_DIR}/cmake/InstallPtiffCli.cmake"
            @ONLY)
        install(SCRIPT "${CMAKE_CURRENT_BINARY_DIR}/cmake/InstallPtiffCli.cmake")
    else()
        message(WARNING "PTIFF_BUILD_CLI=ON but cargo was not found; the ptiff CLI will not be built/installed.")
    endif()
endif()

# ---------------------------------------------------------------------------
# Packaging (CPack)
# ---------------------------------------------------------------------------
set(CPACK_PACKAGE_NAME "libptiff")
set(CPACK_PACKAGE_VENDOR "PTIFF project")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Planetary TIFF (PTIFF) image format library")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://afeldman.github.io/ptiff/")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})
set(CPACK_PACKAGE_CONTACT "anton.feldmann@example.invalid")

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
    set(CPACK_PACKAGE_FILE_NAME "libptiff-${PROJECT_VERSION}-Linux")
elseif(APPLE)
    # macOS: a flat installer (.pkg) via the productbuild generator, with a
    # /usr/local staging prefix so all libs, headers and the CLI land together.
    set(CPACK_PACKAGING_INSTALL_PREFIX "/usr/local")
    set(CPACK_GENERATOR "productbuild")
    set(CPACK_PACKAGE_FILE_NAME "libptiff-${PROJECT_VERSION}-macOS")
endif()

set(CPACK_STRIP_FILES ON)
set(CPACK_SOURCE_IGNORE_FILES
    "build/;build-rel/;build-debug/;.git/;.gitignore;.github/;.nojekyll"
    "/ptiff-cli/target")

include(CPack)
