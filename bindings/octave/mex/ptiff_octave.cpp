// ptiff_octave.cpp
//
// Phase-10 hand-written GNU Octave binding for PTIFF. This is the
// "ptiff-octave" adapter: it calls the C ABI (libptiff_c) DIRECTLY -- it does
// **not** go through SWIG. It is the ONLY Octave binding; the old low-level
// SWIG module (bindings/octave/lib/ptiff.oct) was removed in favour of this
// MEX adapter.
//
//   Octave -> ptiff_octave.oct (this file, native C++ .oct) -> C ABI -> Rust
//
// Octave's own GNU extension mechanism (DEFUN_DLD / the C++ .oct API) is the
// MEX-style C++ shell the plan (§10) calls for: it compiles with mkoctfile,
// speaks only Octave's native `octave_value` types, and links the same stable
// C ABI as every other language binding.
//
// Design
// ------
// * Single entry point `ptiff_octave` (the .oct); the first argument is a
//   command string, the rest are args. A thin `.m` layer (ptiff_open.m,
//   ptiff_read_tile.m, ...) wraps it in an idiomatic API (plan §10.3).
// * Resource handles: opaque C-ABI pointers do not survive the .oct boundary
//   as Octave types, so we keep a process-global registry (uint64 -> resource).
//   `open`/`create` return a uint64 handle that later calls dereference;
//   `close` releases it.
// * Errors: any C-ABI failure or bad argument raises an Octave error() with a
//   ptiff:... identifier so `try/catch` in .m works uniformly (the same
//   contract the other bindings use).

#include <octave/oct.h>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <unordered_map>

// cbindgen's target/ptiff_c.h is plain C (no `extern "C"` wrapper); a C++
// translation unit must wrap it so the ptiff_* symbols resolve to the C
// linkage the Rust `extern "C"` declarations authorise (otherwise the symbols
// would be name-mangled and fail to resolve at dlopen).
#define PTIFF_C_STATIC_DEFINE 1
extern "C" {
#include <ptiff_c.h>
}


namespace {

// ---------------------------------------------------------------------------
// Handle registry
// ---------------------------------------------------------------------------
enum class ResKind { None, Source, Sink };

struct Resource {
  ResKind kind = ResKind::None;
  void *ptr = nullptr;
};

using Registry = std::unordered_map<uint64_t, Resource>;
Registry &registry() {
  static Registry reg;
  return reg;
}
uint64_t next_handle = 1;

uint64_t alloc_resource(Resource r) {
  uint64_t h = next_handle++;
  registry()[h] = std::move(r);
  return h;
}

Resource *lookup(uint64_t h) {
  auto it = registry().find(h);
  return (it == registry().end()) ? nullptr : &it->second;
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------
bool is_char(const octave_value &v) {
  return v.is_string();
}

std::string get_string(const octave_value &v, const char *what) {
  if (!v.is_string())
    error("ptiff:Type", "%s must be a string", what);
  return v.string_value();
}

uint64_t get_handle(const octave_value &v, const char *what) {
  if (v.is_uint64_type() && v.numel() == 1)
    return static_cast<uint64_t>(v.uint64_scalar_value());
  if (v.is_real_scalar())
    return static_cast<uint64_t>(v.scalar_value());
  error("ptiff:Type", "%s must be a uint64 handle", what);
  return 0;
}

uint32_t as_u32(const octave_value &v, const char *what) {
  if (v.numel() != 1 || !v.isnumeric())
    error("ptiff:Type", "%s must be a scalar", what);
  return static_cast<uint32_t>(v.uint64_scalar_value());
}

octave_value make_handle(uint64_t h) { return octave_value(octave_uint64(h)); }

octave_value make_scalar(double v) { return octave_value(v); }

// Convert a C row-major double array into a 1xN Octave row vector.
octave_value matrix_row(const double *arr, size_t n) {
  Matrix m(1, static_cast<octave_idx_type>(n));
  for (size_t i = 0; i < n; ++i)
    m(0, static_cast<octave_idx_type>(i)) = arr[i];
  return octave_value(m);
}

Resource *require_source(uint64_t h) {
  Resource *r = lookup(h);
  if (!r || r->kind != ResKind::Source)
    error("ptiff:Handle", "not a valid open source handle");
  return r;
}
Resource *require_sink(uint64_t h) {
  Resource *r = lookup(h);
  if (!r || r->kind != ResKind::Sink)
    error("ptiff:Handle", "not a valid open sink handle");
  return r;
}

void check_rc(int32_t rc, const char *op) {
  if (rc != 0)
    error("ptiff:ABI", "%s failed (rc=%d)", op, static_cast<int>(rc));
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
// octave::Cell row vector of strings -> Octave cell array of strings.
octave_value string_cell(const std::vector<std::string> &vec) {
  Cell c(1, vec.size());
  for (size_t i = 0; i < vec.size(); ++i)
    c(i) = octave_value(vec[i]);
  return octave_value(c);
}

octave_value cmd_version() {
  ptiff_version c = ptiff_compile_time_version();
  ptiff_version r = ptiff_runtime_version();
  octave_scalar_map m;
  m.setfield("compile_major", octave_value(c.major));
  m.setfield("compile_minor", octave_value(c.minor));
  m.setfield("compile_patch", octave_value(c.patch));
  m.setfield("runtime_major", octave_value(r.major));
  m.setfield("runtime_minor", octave_value(r.minor));
  m.setfield("runtime_patch", octave_value(r.patch));
  return octave_value(m);
}

std::string cmd_backend_names() {
  char *names = ptiff_backend_names();
  if (!names) error("ptiff:ABI", "ptiff_backend_names returned NULL");
  std::string out(names);
  ptiff_free_string(names);
  return out;
}

uint64_t cmd_open(const std::string &path) {
  int32_t rc = 0;
  ptiff_source *src = ptiff_source_open(path.c_str(), &rc);
  if (!src)
    error("ptiff:Open", "could not open '%s' (rc=%d)", path.c_str(),
          static_cast<int>(rc));
  Resource r;
  r.kind = ResKind::Source;
  r.ptr = src;
  return alloc_resource(std::move(r));
}

void cmd_source_close(uint64_t h) {
  Resource *r = lookup(h);
  if (r && r->kind == ResKind::Source) {
    ptiff_source_close(static_cast<ptiff_source *>(r->ptr));
    registry().erase(h);
  }
}

octave_value cmd_source_info(uint64_t h) {
  Resource *r = require_source(h);
  ptiff_source *s = static_cast<ptiff_source *>(r->ptr);
  ptiff_image_descriptor d{};
  check_rc(ptiff_source_descriptor(s, &d), "ptiff_source_descriptor");
  octave_scalar_map m;
  m.setfield("width", octave_value(d.width));
  m.setfield("height", octave_value(d.height));
  m.setfield("pixel_type", octave_value(d.pixel_type));
  m.setfield("channel_count", octave_value(d.channel_count));
  m.setfield("tile_columns", octave_value(ptiff_source_tile_columns(s)));
  m.setfield("tile_rows", octave_value(ptiff_source_tile_rows(s)));
  m.setfield("tile_byte_size", octave_value(ptiff_source_tile_byte_size(s)));
  m.setfield("tile_width", octave_value(d.tile_info.tile_width));
  m.setfield("tile_height", octave_value(d.tile_info.tile_height));
  return octave_value(m);
}

std::string cmd_read_tile(uint64_t h, uint32_t col, uint32_t row) {
  Resource *r = require_source(h);
  ptiff_source *s = static_cast<ptiff_source *>(r->ptr);
  size_t bs = ptiff_source_tile_byte_size(s);
  std::vector<uint8_t> buf(bs);
  size_t nread = 0;
  check_rc(ptiff_source_read_tile(s, col, row, buf.data(), buf.size(), &nread),
            "ptiff_source_read_tile");
  return std::string(reinterpret_cast<char *>(buf.data()), nread);
}

uint64_t cmd_create(const std::string &path, uint32_t w, uint32_t h,
                    uint32_t pixel_type, uint32_t channels, uint32_t tw,
                    uint32_t th,
                    const ptiff_camera *cam) {
  ptiff_image_descriptor d{};
  d.width = w;
  d.height = h;
  d.pixel_type = static_cast<int32_t>(pixel_type);
  d.channel_count = channels;
  d.has_tile_info = 1;
  d.tile_info.tile_width = tw;
  d.tile_info.tile_height = th;
  ptiff_sink *sink = cam ? ptiff_sink_create_camera(path.c_str(), &d, cam)
                          : ptiff_sink_create(path.c_str(), &d);
  if (!sink)
    error("ptiff:Create", "ptiff_sink_create failed for '%s'", path.c_str());
  Resource r;
  r.kind = ResKind::Sink;
  r.ptr = sink;
  return alloc_resource(std::move(r));
}

// Convert an (optional) camera struct argument into a ptiff_camera.
// `arg` must be a scalar struct or empty ([]); returns true if it was present.
bool camera_from_arg(const octave_value &arg, ptiff_camera *out) {
  if (arg.isempty()) return false;
  if (!arg.isstruct() || arg.numel() != 1)
    error("ptiff:Type", "camera must be a scalar struct or []");
  octave_scalar_map m = arg.scalar_map_value();
  *out = ptiff_camera{};
  out->has_intrinsics = m.isfield("has_intrinsics") && m.contents("has_intrinsics").uint_value() != 0;
  out->has_extrinsics = m.isfield("has_extrinsics") && m.contents("has_extrinsics").uint_value() != 0;
  if (m.isfield("focal_length_x")) out->focal_length_x = m.contents("focal_length_x").double_value();
  if (m.isfield("focal_length_y")) out->focal_length_y = m.contents("focal_length_y").double_value();
  if (m.isfield("principal_x")) out->principal_x = m.contents("principal_x").double_value();
  if (m.isfield("principal_y")) out->principal_y = m.contents("principal_y").double_value();
  if (m.isfield("rotation_w")) out->rotation_w = m.contents("rotation_w").double_value();
  if (m.isfield("rotation_x")) out->rotation_x = m.contents("rotation_x").double_value();
  if (m.isfield("rotation_y")) out->rotation_y = m.contents("rotation_y").double_value();
  if (m.isfield("rotation_z")) out->rotation_z = m.contents("rotation_z").double_value();
  if (m.isfield("position_x")) out->position_x = m.contents("position_x").double_value();
  if (m.isfield("position_y")) out->position_y = m.contents("position_y").double_value();
  if (m.isfield("position_z")) out->position_z = m.contents("position_z").double_value();
  if (m.isfield("timestamp")) {
    std::string ts = m.contents("timestamp").string_value();
    std::strncpy(out->timestamp, ts.c_str(), sizeof(out->timestamp) - 1);
    out->timestamp[sizeof(out->timestamp) - 1] = '\0';
  }
  return true;
}

octave_value cmd_camera(const std::string &path) {
  ptiff_camera c{};
  check_rc(ptiff_open_path_camera(path.c_str(), &c), "ptiff_open_path_camera");
  octave_scalar_map m;
  m.setfield("has_intrinsics", octave_value(c.has_intrinsics));
  m.setfield("focal_length_x", octave_value(c.focal_length_x));
  m.setfield("focal_length_y", octave_value(c.focal_length_y));
  m.setfield("principal_x", octave_value(c.principal_x));
  m.setfield("principal_y", octave_value(c.principal_y));
  m.setfield("has_extrinsics", octave_value(c.has_extrinsics));
  m.setfield("rotation_w", octave_value(c.rotation_w));
  m.setfield("rotation_x", octave_value(c.rotation_x));
  m.setfield("rotation_y", octave_value(c.rotation_y));
  m.setfield("rotation_z", octave_value(c.rotation_z));
  m.setfield("position_x", octave_value(c.position_x));
  m.setfield("position_y", octave_value(c.position_y));
  m.setfield("position_z", octave_value(c.position_z));
  m.setfield("timestamp", octave_value(std::string(c.timestamp)));
  // Derived matrices (row-major, as the C ABI presents them). Only set when
  // the corresponding field group is present in the file.
  if (c.has_intrinsics) {
    m.setfield("intrinsics", matrix_row(c.intrinsics, 9));
  }
  if (c.has_extrinsics) {
    m.setfield("extrinsics", matrix_row(c.extrinsics, 12));
  }
  if (c.has_intrinsics && c.has_extrinsics) {
    m.setfield("projection", matrix_row(c.projection, 12));
  }
  return octave_value(m);
}

void cmd_write_tile(uint64_t h, uint32_t col, uint32_t row,
                    const std::string &data) {
  Resource *r = require_sink(h);
  ptiff_sink *s = static_cast<ptiff_sink *>(r->ptr);
  size_t n = data.size();
  size_t bs = ptiff_sink_tile_byte_size(s);
  if (n != bs)
    error("ptiff:TileSize", "tile data has %zu bytes, expected %zu", n, bs);
  check_rc(ptiff_sink_write_tile(s, col, row,
                                  reinterpret_cast<const uint8_t *>(data.data()),
                                  n),
            "ptiff_sink_write_tile");
}

void cmd_sink_close(uint64_t h) {
  Resource *r = lookup(h);
  if (r && r->kind == ResKind::Sink) {
    ptiff_sink_close(static_cast<ptiff_sink *>(r->ptr));
    registry().erase(h);
  }
}

octave_value cmd_sink_info(uint64_t h) {
  Resource *r = require_sink(h);
  ptiff_sink *s = static_cast<ptiff_sink *>(r->ptr);
  octave_scalar_map m;
  m.setfield("tile_columns", octave_value(ptiff_sink_tile_columns(s)));
  m.setfield("tile_rows", octave_value(ptiff_sink_tile_rows(s)));
  m.setfield("tile_byte_size", octave_value(ptiff_sink_tile_byte_size(s)));
  return octave_value(m);
}

octave_value cmd_open_info(const std::string &path) {
  ptiff_image_descriptor d{};
  check_rc(ptiff_open_path(path.c_str(), &d), "ptiff_open_path");
  octave_scalar_map m;
  m.setfield("width", octave_value(d.width));
  m.setfield("height", octave_value(d.height));
  m.setfield("pixel_type", octave_value(d.pixel_type));
  m.setfield("channel_count", octave_value(d.channel_count));
  m.setfield("has_gsd", octave_value(d.has_gsd));
  m.setfield("gsd", octave_value(d.gsd));
  m.setfield("has_tile_info", octave_value(d.has_tile_info));
  m.setfield("tile_width", octave_value(d.tile_info.tile_width));
  m.setfield("tile_height", octave_value(d.tile_info.tile_height));
  m.setfield("has_compression", octave_value(d.has_compression));
  m.setfield("compression", octave_value(d.compression));
  return octave_value(m);
}

octave_value cmd_fields(const std::string &path) {
  ptiff_field *arr = nullptr;
  int count = 0;
  check_rc(ptiff_open_path_fields(path.c_str(), &arr, &count),
            "ptiff_open_path_fields");
  std::vector<std::string> kvec, vvec;
  kvec.reserve(count);
  vvec.reserve(count);
  for (int i = 0; i < count; ++i) {
    kvec.push_back(arr[i].key ? std::string(arr[i].key) : std::string());
    vvec.push_back(arr[i].value ? std::string(arr[i].value) : std::string());
  }
  if (arr) ptiff_fields_free(arr, count);
  octave_scalar_map out;
  out.setfield("keys", string_cell(kvec));
  out.setfield("values", string_cell(vvec));
  return octave_value(out);
}

int32_t cmd_logger_level(const octave_value_list &args, int64_t idx) {
  if (static_cast<size_t>(idx) < args.length()) {
    int32_t lvl = static_cast<int32_t>(as_u32(args(idx), "level"));
    ptiff_logger_set_level(lvl);
    return lvl;
  }
  return ptiff_logger_level();
}

void cmd_logger_log(int32_t level, const std::string &msg) {
  ptiff_logger_log(level, msg.c_str());
}

void cmd_clear() {
  for (auto &kv : registry()) {
    if (kv.second.kind == ResKind::Source)
      ptiff_source_close(static_cast<ptiff_source *>(kv.second.ptr));
    else if (kv.second.kind == ResKind::Sink)
      ptiff_sink_close(static_cast<ptiff_sink *>(kv.second.ptr));
  }
  registry().clear();
  next_handle = 1;
}

}  // namespace

// The exported entry point. `args` indices: in Octave 11 the first real
// argument is args(0). We dispatch on args(0) being the command string.
DEFUN_DLD (ptiff_octave, args, nargout,
            "ptiff_octave(command, ...) -- Phase-10 PTIFF C-ABI adapter.")
{
  if (args.length() == 0)
    error("ptiff:Usage", "ptiff_octave(command, ...)");

  octave_value_list retval;

  std::string cmd = get_string(args(0), "command");

  if (cmd == "version") {
    retval(0) = cmd_version();
  } else if (cmd == "abi_version") {
    retval(0) = octave_value(static_cast<double>(PTIFF_ABI_VERSION));
  } else if (cmd == "backend_names") {
    retval(0) = octave_value(cmd_backend_names());
  } else if (cmd == "open") {
    if (args.length() < 2) error("ptiff:Usage", "open(path)");
    retval(0) = make_handle(cmd_open(get_string(args(1), "path")));
  } else if (cmd == "source_close") {
    if (args.length() < 2) error("ptiff:Usage", "source_close(handle)");
    cmd_source_close(get_handle(args(1), "handle"));
  } else if (cmd == "source_info") {
    if (args.length() < 2) error("ptiff:Usage", "source_info(handle)");
    retval(0) = cmd_source_info(get_handle(args(1), "handle"));
  } else if (cmd == "read_tile") {
    if (args.length() < 4) error("ptiff:Usage", "read_tile(handle, col, row)");
    uint64_t h = get_handle(args(1), "handle");
    uint32_t col = as_u32(args(2), "column");
    uint32_t row = as_u32(args(3), "row");
    retval(0) = octave_value(cmd_read_tile(h, col, row));
  } else if (cmd == "create") {
    if (args.length() < 8) {
      error("ptiff:Usage",
            "create(path, w, h, pixel_type, channels, tile_w, tile_h [, camera])");
    }
    ptiff_camera cam{};
    const ptiff_camera *cam_ptr = nullptr;
    if (args.length() >= 9 && camera_from_arg(args(8), &cam))
      cam_ptr = &cam;
    retval(0) = make_handle(cmd_create(
        get_string(args(1), "path"), as_u32(args(2), "width"),
        as_u32(args(3), "height"), as_u32(args(4), "pixel_type"),
        as_u32(args(5), "channels"), as_u32(args(6), "tile_width"),
        as_u32(args(7), "tile_height"), cam_ptr));
  } else if (cmd == "write_tile") {
    if (args.length() < 5) {
      error("ptiff:Usage", "write_tile(handle, col, row, data)");
    }
    uint64_t h = get_handle(args(1), "handle");
    uint32_t col = as_u32(args(2), "column");
    uint32_t row = as_u32(args(3), "row");
    std::string data;
    if (args(4).is_string()) {
      data = args(4).string_value();
    } else if (args(4).is_uint8_type()) {
      // Accept a uint8 vector as tile bytes.
      uint8NDArray a = args(4).uint8_array_value();
      data.assign(reinterpret_cast<char *>(a.fortran_vec()), a.numel());
    } else if (args(4).is_real_matrix()) {
      NDArray a = args(4).array_value();
      data.assign(reinterpret_cast<char *>(a.fortran_vec()), a.numel());
    } else {
      error("ptiff:Type", "tile data must be a string or numeric array");
    }
    cmd_write_tile(h, col, row, data);
  } else if (cmd == "sink_close") {
    if (args.length() < 2) error("ptiff:Usage", "sink_close(handle)");
    cmd_sink_close(get_handle(args(1), "handle"));
  } else if (cmd == "sink_info") {
    if (args.length() < 2) error("ptiff:Usage", "sink_info(handle)");
    retval(0) = cmd_sink_info(get_handle(args(1), "handle"));
  } else if (cmd == "open_info") {
    if (args.length() < 2) error("ptiff:Usage", "open_info(path)");
    retval(0) = cmd_open_info(get_string(args(1), "path"));
  } else if (cmd == "fields") {
    if (args.length() < 2) error("ptiff:Usage", "metadata(path)");
    retval(0) = cmd_fields(get_string(args(1), "path"));
  } else if (cmd == "camera") {
    if (args.length() < 2) error("ptiff:Usage", "camera(path)");
    retval(0) = cmd_camera(get_string(args(1), "path"));
  } else if (cmd == "logger_level") {
    retval(0) = octave_value(
        cmd_logger_level(args, 1));
  } else if (cmd == "logger_log") {
    if (args.length() < 3) error("ptiff:Usage", "logger_log(level, message)");
    int32_t level = static_cast<int32_t>(as_u32(args(1), "level"));
    cmd_logger_log(level, get_string(args(2), "message"));
  } else if (cmd == "clear") {
    cmd_clear();
  } else {
    error("ptiff:Command", "unknown command '%s'", cmd.c_str());
  }

  return retval;
}
