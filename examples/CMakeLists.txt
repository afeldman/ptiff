# examples/CMakeLists.txt
#
# Builds the local, self-contained examples against the installed/exported
# `ptiff::ptiff` target. This directory is added from the repo-root
# CMakeLists.txt when `PTIFF_BUILD_EXAMPLES=ON`.
#
# The three `*_local_sample` targets generate / inspect / copy small synthetic
# PTIFF files on the fly and need no downloaded data or network access.

# 1. Write a small synthetic PTIFF (128x128 UInt8, pinhole camera calibration).
add_executable(ptiff_example_write_local_sample write_local_sample.cpp)
target_link_libraries(ptiff_example_write_local_sample PRIVATE ptiff::ptiff)
target_compile_features(ptiff_example_write_local_sample PRIVATE cxx_std_23)

# 2. Inspect a PTIFF: metadata + camera calibration + first tile.
add_executable(ptiff_example_read_local_sample read_local_sample.cpp)
target_link_libraries(ptiff_example_read_local_sample PRIVATE ptiff::ptiff)
target_compile_features(ptiff_example_read_local_sample PRIVATE cxx_std_23)

# 3. Copy a PTIFF, pixel-for-pixel and preserving the camera calibration.
add_executable(ptiff_example_copy_local_sample copy_local_sample.cpp)
target_link_libraries(ptiff_example_copy_local_sample PRIVATE ptiff::ptiff)
target_compile_features(ptiff_example_copy_local_sample PRIVATE cxx_std_23)

# Optional: ESA/NASA Earth-observation TIFF reader. Requires downloading real
# granules via examples/fetch_esa_nasa_data.py (network access; large files).
add_executable(ptiff_example_read_esa_nasa_tiff read_esa_nasa_tiff.cpp)
target_link_libraries(ptiff_example_read_esa_nasa_tiff PRIVATE ptiff::ptiff)
target_compile_features(ptiff_example_read_esa_nasa_tiff PRIVATE cxx_std_23)
