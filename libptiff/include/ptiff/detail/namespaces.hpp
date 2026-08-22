#pragma once

/// Reserved internal namespaces. Declared here, with no members yet, purely so the names are
/// claimed and documented before any code needs them -- avoids ad hoc namespace choices once
/// a domain (compression codecs, camera-specific internals, shared low-level utilities) grows
/// real content. Add real declarations directly in a dedicated header/source under the matching
/// src/\<name\>/ directory when that day comes; do not add files here.
///
/// - `ptiff::camera` -- internal camera-model representations, distinct from the more
///   general `ptiff::geometry` (CRS/transform primitives).
/// - `ptiff::compression` -- internal codec implementations backing TIFF compression
///   scheme support.
/// - `ptiff::utility` -- cross-cutting internal helpers (span/string utilities, concepts)
///   with no home in a specific domain.

namespace ptiff::camera {}
namespace ptiff::compression {}
namespace ptiff::utility {}
