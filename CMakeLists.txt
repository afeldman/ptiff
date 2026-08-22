cmake_minimum_required(VERSION 3.26)

project(
    PTIFF
    VERSION 0.4.0
    LANGUAGES CXX
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
