#pragma once

/// @file ptiff.hpp
/// @brief Umbrella header pulling in the full libptiff public API.
///
/// Including this single header gives access to every public symbol documented in the PTIFF
/// reference API: core error/result types, geometry, images, scenes, metadata, the reader/
/// writer IO facade and all registered storage backends.
///
/// For the current implementation status per backend/transport, see `ARCHITECTURE.md` -- this
/// comment intentionally does not restate it here to avoid drifting out of sync.

#include <ptiff/core/error.hpp>
#include <ptiff/core/precondition.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/core/version.hpp>
#include <ptiff/export.hpp>
#include <ptiff/geometry/camera.hpp>
#include <ptiff/geometry/coordinate_reference_system.hpp>
#include <ptiff/image.hpp>
#include <ptiff/io/backend/isis_backend.hpp>
#include <ptiff/io/backend/memory_backend.hpp>
#include <ptiff/io/backend/openexr_backend.hpp>
#include <ptiff/io/backend/pds4_backend.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/backend/zarr_backend.hpp>
#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/deserializer.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/serializer.hpp>
#include <ptiff/io/storage_backend.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_cache.hpp>
#include <ptiff/io/tile/tile_iterator.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile/tile_storage.hpp>
#include <ptiff/io/writer.hpp>
#include <ptiff/logging/logger.hpp>
#include <ptiff/metadata/metadata.hpp>
#include <ptiff/metadata/scientific_layer.hpp>
#include <ptiff/scene.hpp>
