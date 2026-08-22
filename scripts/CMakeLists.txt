# Fixture generator for the interop/overhead experiment behind
# documents/paper/ptiff/ptiff.tex Section "Empirische Interoperabilitäts- und
# Overhead-Messung". Gated behind PTIFF_BUILD_INTEROP_SCRIPTS (default OFF) --
# not part of the release build.
# Backend stubs self-register with BackendFactory via a static initializer with no
# externally visible symbol -- WHOLE_ARCHIVE forces every object file in ptiff to be
# linked in so the registrations actually run (same pattern as
# libptiff/tests/unit/CMakeLists.txt).
add_executable(gen_interop_fixture gen_interop_fixture.cpp)
target_link_libraries(gen_interop_fixture PRIVATE $<LINK_LIBRARY:WHOLE_ARCHIVE,ptiff::ptiff>)

# P-6: real-data test (genuine LRO NAC-derived DTM read, real LOLA elevation roundtrip).
add_executable(test_real_data test_real_data.cpp)
target_link_libraries(test_real_data PRIVATE $<LINK_LIBRARY:WHOLE_ARCHIVE,ptiff::ptiff>)
