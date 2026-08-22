# Resolves a short git describe hash for embedding into the generated version header.
# Falls back to "unknown" when not building from a git checkout (e.g. release tarballs).
function(ptiff_git_version output_variable)
    set(hash "unknown")

    find_package(Git QUIET)
    if(GIT_FOUND AND EXISTS "${CMAKE_SOURCE_DIR}/.git")
        execute_process(
            COMMAND ${GIT_EXECUTABLE} describe --always --dirty --abbrev=12
            WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
            OUTPUT_VARIABLE git_output
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
            RESULT_VARIABLE git_result
        )
        if(git_result EQUAL 0 AND NOT "${git_output}" STREQUAL "")
            set(hash "${git_output}")
        endif()
    endif()

    set(${output_variable} "${hash}" PARENT_SCOPE)
endfunction()
