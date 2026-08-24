#pragma once

/// @file ptiff.hpp
/// @brief Umbrella header for the `ptiff-cpp` wrapper over the PTIFF C ABI.
///
/// Including this single header pulls in the full `ptiff-cpp` public surface:
/// core error/result/id types, image metadata, scene, camera, the reader/
/// writer IO facade and version accessors — all using the type names the
/// historical C++ reference library exposed (`Image`, `Scene`, `Reader`,
/// `Writer`, `Result<T>`, `Error`, `ErrorCode`, `ImageDescriptor`, `Camera`).
///
/// The wrapper speaks the stable C ABI (`libptiff_c`, built from
/// `crates/ptiff-c`); link with `-lptiff_c` (see `bindings/cpp/Makefile`).

#include <ptiff/core.hpp>
#include <ptiff/version.hpp>
#include <ptiff/image.hpp>
#include <ptiff/scene.hpp>
#include <ptiff/camera.hpp>
#include <ptiff/source.hpp>
#include <ptiff/reader.hpp>
#include <ptiff/writer.hpp>
