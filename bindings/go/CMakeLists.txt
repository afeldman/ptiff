# CMake integration for the Go bindings (bindings/go).
#
# Guarded by PTIFF_BUILD_GO_BINDINGS. Best results with BUILD_SHARED_LIBS=ON
# (a self-contained libptiff.dylib bundles its C++ deps, and the process-wide
# BackendFactory self-registration is fully visible across the FFI boundary).
#
# bindings/go/ptiff is PROMOTED SWIG output (see the "alles in SWIG" migration
# notes in bindings/swig/README.md): it is the raw C-ABI-shaped module SWIG
# generates from bindings/swig/ptiff.i, regenerated here rather than hand-
# written. It never compiles C++ itself -- the generated wrapper is plain C,
# bound only to the extern "C" surface of libptiff_c, exactly like the other
# language bindings.
#
# When enabled this:
#   * runs `make -C bindings/swig go` (the single source of truth for all
#     three SWIG language targets) with PTIFF_C_LIB_DIR pointed at the
#     current configuration's libptiff_c build output, regenerating
#     bindings/go/ptiff/{ptiff.go,ptiff_wrap.c,cgo_flags.go};
#   * registers CTests (ptiff_go_bindings_generate, ptiff_go_bindings) that
#     regenerate then `go test`.
#
# Requires a Go toolchain (cgo + a C compiler), SWIG, and `make` on PATH.

if(NOT PTIFF_BUILD_GO_BINDINGS)
    return()
endif()

find_program(GO_EXECUTABLE go)
if(NOT GO_EXECUTABLE)
    message(WARNING "PTIFF_BUILD_GO_BINDINGS is ON but no 'go' found on PATH; "
                    "skipping Go binding build.")
    return()
endif()

find_program(SWIG_EXECUTABLE swig)
if(NOT SWIG_EXECUTABLE)
    message(WARNING "PTIFF_BUILD_GO_BINDINGS is ON but no 'swig' found on PATH; "
                    "skipping Go binding build.")
    return()
endif()

find_program(MAKE_EXECUTABLE NAMES make gmake)
if(NOT MAKE_EXECUTABLE)
    message(WARNING "PTIFF_BUILD_GO_BINDINGS is ON but no 'make' found on PATH; "
                    "skipping Go binding build.")
    return()
endif()

# The Go shim needs libptiff_c (the C ABI). Enabling the Go binding forces it on
# in the root CMakeLists.txt; assert it here for clarity / early failure.
if(NOT TARGET ptiff_c)
    message(FATAL_ERROR "PTIFF_BUILD_GO_BINDINGS requires PTIFF_BUILD_C_BINDINGS "
                        "(target ptiff_c) to be enabled.")
endif()

set(_swig_dir "${CMAKE_CURRENT_SOURCE_DIR}/../swig")

# Regenerate bindings/go/ptiff/{ptiff.go,ptiff_wrap.c,cgo_flags.go} from
# bindings/swig/ptiff.i via the shared Makefile. PTIFF_C_LIB_DIR is a generator
# expression so it resolves to the libptiff_c build output of the CURRENT
# configuration (Debug/Release/...), not a hardcoded path.
add_test(NAME ptiff_go_bindings_generate
    COMMAND ${CMAKE_COMMAND} -E env "PTIFF_C_LIB_DIR=$<TARGET_FILE_DIR:ptiff_c>"
            ${MAKE_EXECUTABLE} -C ${_swig_dir} go
)

add_test(NAME ptiff_go_bindings
    COMMAND ${GO_EXECUTABLE} test -count=1 ./ptiff
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(ptiff_go_bindings PROPERTIES
    TIMEOUT 300
    DEPENDS ptiff_go_bindings_generate
)
