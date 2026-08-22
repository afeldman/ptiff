# PTIFF conformance tests (executable).
#
# This is the executable mirror of the normative suite in <repo-root>/conformance/.
# It currently covers Level 0 (baseline / container) -- TIFF and BigTIFF -- per
# conformance/levels.md and conformance/matrix.md. Extension-domain suites
# (camera, CRS, SPICE, ...) will be added here as their RFCs are ratified.
#
# The tests need the same preconditions as the unit/golden suites: the internal
# headers under libptiff/include/ptiff/ and whole-archive linking so the
# TiffBackend self-registers with BackendFactory.

add_executable(ptiff_conformance_tests
    baseline_tiff_conformance_test.cpp
)

target_include_directories(ptiff_conformance_tests PRIVATE
    ${CMAKE_SOURCE_DIR}/libptiff/include
)

target_link_libraries(ptiff_conformance_tests
    PRIVATE
        $<LINK_LIBRARY:WHOLE_ARCHIVE,ptiff::ptiff>
        Catch2::Catch2WithMain
        ptiff_warnings
)

catch_discover_tests(ptiff_conformance_tests)
