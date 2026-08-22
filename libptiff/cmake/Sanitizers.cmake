# Provides ptiff_enable_sanitizers(<target>), applying compile/link flags for whichever of
# PTIFF_ENABLE_ASAN / PTIFF_ENABLE_UBSAN / PTIFF_ENABLE_TSAN cache options are ON.
# ASAN and TSAN are mutually exclusive; UBSAN may be combined with either.

function(ptiff_enable_sanitizers target)
    if(PTIFF_ENABLE_ASAN AND PTIFF_ENABLE_TSAN)
        message(FATAL_ERROR "PTIFF_ENABLE_ASAN and PTIFF_ENABLE_TSAN are mutually exclusive")
    endif()

    set(sanitizer_flags "")

    if(PTIFF_ENABLE_ASAN)
        list(APPEND sanitizer_flags "-fsanitize=address")
    endif()

    if(PTIFF_ENABLE_UBSAN)
        list(APPEND sanitizer_flags "-fsanitize=undefined")
    endif()

    if(PTIFF_ENABLE_TSAN)
        list(APPEND sanitizer_flags "-fsanitize=thread")
    endif()

    if(sanitizer_flags)
        target_compile_options(${target} PRIVATE ${sanitizer_flags} -fno-omit-frame-pointer)
        target_link_options(${target} PRIVATE ${sanitizer_flags})
    endif()
endfunction()
