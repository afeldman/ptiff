find_package(Catch2 3 REQUIRED)

add_executable(ptiff_benchmarks main.cpp)

target_link_libraries(ptiff_benchmarks
    PRIVATE
        ptiff::ptiff
        Catch2::Catch2WithMain
        ptiff_warnings
)
