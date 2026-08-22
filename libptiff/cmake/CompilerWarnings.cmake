# Defines an INTERFACE target `ptiff_warnings` carrying warnings-as-conscientious-defaults flags.
# Link it PRIVATE into any target that should be built with these warnings enabled.

add_library(ptiff_warnings INTERFACE)

if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(ptiff_warnings INTERFACE
        -Wall
        -Wextra
        -Wpedantic
        -Wshadow
        -Wconversion
        -Wsign-conversion
        -Wnon-virtual-dtor
        -Woverloaded-virtual
        -Wnull-dereference
        -Wdouble-promotion
    )
elseif(CMAKE_CXX_COMPILER_ID STREQUAL "MSVC")
    target_compile_options(ptiff_warnings INTERFACE
        /W4
        /permissive-
        /w14263
        /w14264
    )
endif()
