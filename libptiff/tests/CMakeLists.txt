find_package(Catch2 3 REQUIRED)
include(Catch)

# Test-only dependency: local loopback HTTP server for exercising
# ptiff::io::HttpRangeBinaryReader against, without a real network endpoint.
find_package(httplib REQUIRED CONFIG)

add_subdirectory(unit)
add_subdirectory(integration)
add_subdirectory(golden)
add_subdirectory(conformance)
