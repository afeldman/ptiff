# Golden-file conformance tests.
#
# These assert the *stability* of libptiff's on-disk serialization (a format /
# conformance contract) by comparing deterministic output against an agreed,
# in-repo golden SHA-256 digest. They run as a separate Catch2 executable from
# the unit suite so a format drift is visible independently of unit coverage.
#
# A golden test needs the same preconditions as a unit test: the internal
# headers under libptiff/include/ptiff/ and whole-archive linking so backend
# stubs self-register with BackendFactory.

add_executable(ptiff_golden_tests
    tiff_roundtrip_golden_test.cpp
)

target_include_directories(ptiff_golden_tests PRIVATE
    ${CMAKE_SOURCE_DIR}/libptiff/include
)

target_link_libraries(ptiff_golden_tests
    PRIVATE
        $<LINK_LIBRARY:WHOLE_ARCHIVE,ptiff::ptiff>
        Catch2::Catch2WithMain
        ptiff_warnings
)

catch_discover_tests(ptiff_golden_tests)
