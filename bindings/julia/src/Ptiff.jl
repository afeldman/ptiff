# Ptiff.jl
#
# Julia binding for the PTIFF (Planetary TIFF) format, following the
# establishment "no SWIG, call the C ABI directly" pattern used by the other
# language bindings in this repository (see `bindings/README.md`).
#
# Julia has `ccall` natively, so unlike the Octave binding there is no C/C++
# adapter to build: this module calls `libptiff_c` (the stable C ABI generated
# from the Rust core via cbindgen) directly. The only "build" artifact needed
# at runtime is the shared library produced by
#
#     cargo build -p ptiff-c --release
#
# at the workspace root. That crate declares `crate-type = ["cdylib", ...]`,
# so it emits `libptiff_c.{dylib,so}` in `target/release/`, and the cbindgen
# header `target/ptiff_c.h` is the source of truth for every signature below.
#
# Pipeline:
#
#     Julia -> Ptiff.jl (ccall) -> C ABI (libptiff_c) -> Rust core
#
# No dependency on SWIG. The file descriptors / image descriptors / tile data
# all travel over the plain `stdint.h`/`stddef.h` types that the C ABI exposes.

module Ptiff

using Libdl

export
    # opening / reading
    PtiffSource, open_source, source_info, read_tile, close_source,
    # writing
    PtiffSink, create_sink, sink_info, write_tile, close_sink,
    # inspection / metadata / camera
    read_info, read_metadata, read_camera,
    # backends / version / logging
    backends, version, compile_time_version, runtime_version,
    logger_set_level, logger_level, logger_log,
    # constants
    PixelType, PTIFF_PIXEL_UINT8, PTIFF_PIXEL_UINT16, PTIFF_PIXEL_UINT32,
    PTIFF_PIXEL_FLOAT32, PTIFF_PIXEL_FLOAT64,
    Compression, PTIFF_COMPRESSION_NONE, PTIFF_COMPRESSION_LZW,
    PTIFF_COMPRESSION_DEFLATE, PTIFF_COMPRESSION_JPEG,
    LogLevel, LogTrace, LogDebug, LogInfo, LogWarn, LogError, LogCritical, LogOff,
    PTIFFError

# ---------------------------------------------------------------------------
# Library loading
# ---------------------------------------------------------------------------

"""
    Ptiff.libpath(; dir = nothing) -> String

Resolve the path to `libptiff_c`. By default it is looked up in the Rust
workspace `target/release/` relative to this source file (same convention the
Octave MEX Makefile uses). Override with `PTIFF_C_LIB_DIR` or pass `dir`.
"""
function libpath(; dir::Union{Nothing,AbstractString}=nothing)
    if dir === nothing
        dir = get(ENV, "PTIFF_C_LIB_DIR", nothing)
    end
    if dir !== nothing
        # exact file path given?
        if isfile(dir)
            return dir
        end
        dir = abspath(dir)
    else
        # default: <repo root>/target/release relative to this file
        # (src/Ptiff.jl under bindings/julia/src -> repo root is 3 up)
        here = @__DIR__
        root = dirname(dirname(dirname(here)))  # src -> julia -> bindings -> ptiff root
        dir = joinpath(root, "target", "release")
        dir = abspath(dir)
    end
    candidates = [joinpath(dir, "libptiff_c.so"), joinpath(dir, "libptiff_c.dylib")]
    for c in candidates
        isfile(c) && return c
    end
    error("libptiff_c not found under $(dir). Run `cargo build -p ptiff-c --release` in the repo root.")
end

const _lib = Ref{Any}()

function __init__()
    _lib[] = dlopen(libpath(); throw_error=true)
end

"""
    _sym(name) -> Ptr{Nothing}

Resolve `name` inside `libptiff_c` to a function pointer so `ccall` can invoke
it directly. (Julia 1.12 no longer accepts a raw `Ptr` as the library part of
`ccall((name, lib), ...)`, so we pass a dlsym'd function pointer instead.)
"""
_sym(name::Symbol) = Base.Libc.Libdl.dlsym(_lib[], name)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Mirror of the C `ptiff_pixel_type` enum (ordering matches the Rust core).
@enum PixelType::Int32 begin
    PTIFF_PIXEL_UINT8   = 0
    PTIFF_PIXEL_UINT16  = 1
    PTIFF_PIXEL_UINT32  = 2
    PTIFF_PIXEL_FLOAT32 = 3
    PTIFF_PIXEL_FLOAT64 = 4
end

# Mirror of the C `ptiff_compression_kind` enum.
@enum Compression::Int32 begin
    PTIFF_COMPRESSION_NONE   = 0
    PTIFF_COMPRESSION_LZW    = 1
    PTIFF_COMPRESSION_DEFLATE = 2
    PTIFF_COMPRESSION_JPEG   = 3
end

# Mirror of the C `ptiff_log_level` enum.
@enum LogLevel::Int32 begin
    LogTrace   = 0
    LogDebug   = 1
    LogInfo    = 2
    LogWarn    = 3
    LogError   = 4
    LogCritical = 5
    LogOff     = 6
end

# Mirror of the C `ptiff_error_code` enum. The C ABI returns `0` on success
# and a *negative* error code on failure (`-ptiff_error_code`).
@enum ErrorCode::Int32 begin
    ErrorNotImplemented    = 0
    ErrorInvalidArgument   = 1
    ErrorOutOfRange        = 2
    ErrorNotFound          = 3
    ErrorUnknown           = 4
end

"""
    PTIFFError(code::Int32, msg::String)

Thrown when a C ABI call returns a negative error code. `code` is the
positive `ptiff_error_code` value; `msg` is the human-readable operation
context.
"""
struct PTIFFError <: Exception
    code::Int32
    msg::String
end

Base.showerror(io::IO, e::PTIFFError) =
    print(io, "PTIFFError($(_errname(e.code)), $(repr(e.msg)))")

_errname(c::Int32) = if c == Int32(ErrorInvalidArgument)
    "INVALID_ARGUMENT"
elseif c == Int32(ErrorOutOfRange)
    "OUT_OF_RANGE"
elseif c == Int32(ErrorNotFound)
    "NOT_FOUND"
elseif c == Int32(ErrorUnknown)
    "UNKNOWN"
elseif c == Int32(ErrorNotImplemented)
    "NOT_IMPLEMENTED"
else
    "UNKNOWN"
end

# ---------------------------------------------------------------------------
# C structs (C-compatible layouts, matching target/ptiff_c.h)
# ---------------------------------------------------------------------------

# typedef struct ptiff_tile_info { uint32_t tile_width; uint32_t tile_height; }
struct CTileInfo
    tile_width::UInt32
    tile_height::UInt32
end

# typedef struct ptiff_image_descriptor { ... } ptiff_image_descriptor;
struct CImageDescriptor
    width::UInt32
    height::UInt32
    pixel_type::Int32
    channel_count::UInt32
    has_gsd::Int32
    gsd::Float64
    has_tile_info::Int32
    tile_width::UInt32
    tile_height::UInt32
    has_compression::Int32
    compression::Int32
end

# typedef struct ptiff_field { char *key; char *value; }
struct CField
    key::Ptr{Cchar}
    value::Ptr{Cchar}
end

# typedef struct ptiff_version { int32_t major; int32_t minor; int32_t patch; }
struct CVersion
    major::Int32
    minor::Int32
    patch::Int32
end

# typedef struct ptiff_camera { ... } ptiff_camera;
# Fixed arrays sized per K_CAMERA_LENS_MAX_PARAMS == 8.
struct CCamera
    has_intrinsics::Int32
    focal_length_x::Float64
    focal_length_y::Float64
    principal_x::Float64
    principal_y::Float64
    intrinsics::NTuple{9,Float64}
    has_extrinsics::Int32
    rotation_w::Float64
    rotation_x::Float64
    rotation_y::Float64
    rotation_z::Float64
    position_x::Float64
    position_y::Float64
    position_z::Float64
    extrinsics::NTuple{12,Float64}
    projection::NTuple{12,Float64}
    timestamp::NTuple{64,Int8}
    has_lens::Int32
    lens_kind::NTuple{32,Int8}
    lens_param_count::UInt32
    lens_param_key::NTuple{8,NTuple{16,Int8}}
    lens_param_value::NTuple{8,Float64}
end

# ---------------------------------------------------------------------------
# Public high-level result objects
# ---------------------------------------------------------------------------

"""
    ImageDescriptor

Read-only metadata model of a PTIFF image: dimensions, pixel type, channel
count, ground-sample distance, tile size and compression.
"""
struct ImageDescriptor
    width::Int
    height::Int
    pixel_type::PixelType
    channel_count::Int
    gsd::Union{Nothing,Float64}
    tile_width::Union{Nothing,Int}
    tile_height::Union{Nothing,Int}
    compression::Compression
end

"""
    Version

Library version triple.
"""
struct Version
    major::Int
    minor::Int
    patch::Int
end

"""
    PtiffSource

Opaque handle over an open PTIFF file for tile-cursor reading. Owns the
underlying C source; release with [`close_source`].
"""
mutable struct PtiffSource
    ptr::Ptr{Cvoid}
    function PtiffSource(ptr::Ptr{Cvoid})
        ptr == C_NULL && error("PtiffSource: null handle")
        new(ptr)
    end
end

"""
    PtiffSink

Opaque handle over an open PTIFF writer. Release (and flush) with
[`close_sink`]; the file is not readable until it is closed.
"""
mutable struct PtiffSink
    ptr::Ptr{Cvoid}
    tile_columns::Int
    tile_rows::Int
    tile_byte_size::Int
    function PtiffSink(ptr::Ptr{Cvoid})
        ptr == C_NULL && error("PtiffSink: null handle")
        cols = ccall(_sym(:ptiff_sink_tile_columns), UInt32, (Ptr{Cvoid},), ptr)
        rows = ccall(_sym(:ptiff_sink_tile_rows), UInt32, (Ptr{Cvoid},), ptr)
        tbs  = ccall(_sym(:ptiff_sink_tile_byte_size), UInt, (Ptr{Cvoid},), ptr)
        new(ptr, Int(cols), Int(rows), Int(tbs))
    end
end

# ---------------------------------------------------------------------------
# Low-level helpers
# ---------------------------------------------------------------------------

_check(code::Int32, what::AbstractString) = begin
    if code < 0
        throw(PTIFFError(-code, String(what)))
    end
    code
end

_to_union(has::Int32, val) = has != 0 ? val : nothing

# ---------------------------------------------------------------------------
# Reading
# ---------------------------------------------------------------------------

"""
    open_source(path) -> PtiffSource

Open a PTIFF (or any backend-supported) file for tile-cursor reading. Returns
an opaque handle; release it with [`close_source`].
"""
function open_source(path::AbstractString)
    err = Ref{Int32}(0)
    ptr = ccall(_sym(:ptiff_source_open), Ptr{Cvoid},
                (Cstring, Ref{Int32}), path, err)
    ptr == C_NULL && throw(PTIFFError(Int32(-err[]), "open_source($(path))"))
    PtiffSource(ptr)
end

"""
    close_source(source::PtiffSource)

Release the source handle. `NULL` is a no-op.
"""
function close_source(source::PtiffSource)
    ccall(_sym(:ptiff_source_close), Cvoid, (Ptr{Cvoid},), source.ptr)
    source.ptr = C_NULL
    nothing
end

"""
    source_info(source::PtiffSource) -> ImageDescriptor

Read the descriptor of an already-open source (no extra handle retained).
"""
function source_info(source::PtiffSource)
    desc = Ref{CImageDescriptor}()
    code = ccall(_sym(:ptiff_source_descriptor), Int32,
        (Ptr{Cvoid}, Ref{CImageDescriptor}), source.ptr, desc)
    _check(code, "source_info")
    _from_c(desc[])
end

"""
    read_info(path) -> ImageDescriptor

Read an image's descriptor (size/type/tiles/compression) directly from
`path`, without retaining a source handle.
"""
function read_info(path::AbstractString)
    source = open_source(path)
    try
        return source_info(source)
    finally
        close_source(source)
    end
end

"""
    read_tile(source, column, row) -> Vector{UInt8}

Read a single decoded (decompressed) tile at grid `(column, row)` level 0 as
raw sample bytes. The returned vector has exactly the tile byte size (see
`sink_info`/`PtiffSource`); reinterpret it to your pixel type.
"""
function read_tile(source::PtiffSource, column::Integer, row::Integer)
    tbs = ccall(_sym(:ptiff_source_tile_byte_size), UInt, (Ptr{Cvoid},), source.ptr)
    buf = Vector{UInt8}(undef, Int(tbs))
    bytes_read = Ref{UInt}(0)
    code = ccall(_sym(:ptiff_source_read_tile), Int32,
        (Ptr{Cvoid}, UInt32, UInt32, Ptr{UInt8}, UInt, Ref{UInt}),
        source.ptr, UInt32(column), UInt32(row), buf, tbs, bytes_read)
    _check(code, "read_tile")
    resize!(buf, Int(bytes_read[]))
    buf
end

# ---------------------------------------------------------------------------
# Writing
# ---------------------------------------------------------------------------

"""
    create_sink(path, width, height, pixel_type, channel_count=1;
                tile_width=0, tile_height=0, gsd=nothing,
                compression=PTIFF_COMPRESSION_NONE) -> PtiffSink

Create a new PTIFF file for writing. The image must be tiled (`tile_width`/
`tile_height` > 0); the C ABI rejects untiled sinks. `gsd` sets the
ground-sample distance on the in-memory descriptor only — it is
format-neutral and dropped on a TIFF round-trip (see the core
scene_serializer), so it does NOT survive a file write+read. Returns a sink
handle; write tiles with [`write_tile`] and finish with [`close_sink`].
"""
function create_sink(path::AbstractString, width::Integer, height::Integer,
                        pixel_type::PixelType, channel_count::Integer=1;
                        tile_width::Integer=0, tile_height::Integer=0,
                        gsd::Union{Nothing,AbstractFloat}=nothing,
                        compression::Compression=PTIFF_COMPRESSION_NONE)
    has_gsd = gsd === nothing ? Int32(0) : Int32(1)
    gsdv = gsd === nothing ? 0.0 : Float64(gsd)
    has_tile = (tile_width > 0 || tile_height > 0) ? Int32(1) : Int32(0)
    desc = CImageDescriptor(
        UInt32(width), UInt32(height), Int32(pixel_type), UInt32(channel_count),
        has_gsd, gsdv, has_tile, UInt32(tile_width), UInt32(tile_height),
        Int32(1), Int32(compression))
    ptr = ccall(_sym(:ptiff_sink_create), Ptr{Cvoid},
                (Cstring, Ref{CImageDescriptor}), path, desc)
    ptr == C_NULL && error("ptiff_sink_create($(path)): failed")
    PtiffSink(ptr)
end

"""
    sink_info(sink::PtiffSink) -> (tile_columns, tile_rows, tile_byte_size)

Tile grid geometry of an open sink. Stored on the handle at creation.
"""
sink_info(sink::PtiffSink) = (sink.tile_columns, sink.tile_rows, sink.tile_byte_size)

"""
    write_tile(sink, column, row, buffer::Vector{UInt8})

Write one raw, uncompressed tile. `buffer` must be exactly
`sink_info(sink)[3]` bytes.
"""
function write_tile(sink::PtiffSink, column::Integer, row::Integer, buffer::Vector{UInt8})
    code = ccall(_sym(:ptiff_sink_write_tile), Int32,
                    (Ptr{Cvoid}, UInt32, UInt32, Ptr{UInt8}, UInt),
                    sink.ptr, UInt32(column), UInt32(row), buffer, UInt(length(buffer)))
    _check(code, "write_tile")
    nothing
end

"""
    close_sink(sink::PtiffSink)

Flush and release the sink. The file becomes valid for reading only after
this call.
"""
function close_sink(sink::PtiffSink)
    ccall(_sym(:ptiff_sink_close), Cvoid, (Ptr{Cvoid},), sink.ptr)
    sink.ptr = C_NULL
    nothing
end

# ---------------------------------------------------------------------------
# Metadata
# ---------------------------------------------------------------------------

"""
    read_metadata(path) -> Dict{String,String}

Read the flattened metadata fields of `path` into key/value pairs.
"""
function read_metadata(path::AbstractString)
    fields = Ref{Ptr{CField}}()
    count = Ref{Int32}(0)
    code = ccall(_sym(:ptiff_open_path_fields), Int32,
                    (Cstring, Ref{Ptr{CField}}, Ref{Int32}), path, fields, count)
    _check(code, "read_metadata")
    n = Int(count[])
    out = Dict{String,String}()
    if n > 0
        arr = fields[]
        for i in 0:(n - 1)
            f = unsafe_load(arr, i + 1)
            key = unsafe_string(f.key)
            val = unsafe_string(f.value)
            out[key] = val
        end
        ccall(_sym(:ptiff_fields_free), Cvoid, (Ptr{CField}, Int32), arr, count[])
    end
    out
end

# ---------------------------------------------------------------------------
# Camera
# ---------------------------------------------------------------------------

"""
    read_camera(path) -> Dict{String,Any}

Read the structured camera calibration of `path`, or an empty dict when the
file carries no `ptiff.camera.*` fields.
"""
function read_camera(path::AbstractString)
    cam = Ref{CCamera}()
    code = ccall(_sym(:ptiff_open_path_camera), Int32,
                    (Cstring, Ref{CCamera}), path, cam)
    _check(code, "read_camera")
    c = cam[]
    if c.has_intrinsics == 0 && c.has_extrinsics == 0
        return Dict{String,Any}()
    end
    out = Dict{String,Any}()
    if c.has_intrinsics != 0
        out["focal_length_x"] = c.focal_length_x
        out["focal_length_y"] = c.focal_length_y
        out["principal_x"] = c.principal_x
        out["principal_y"] = c.principal_y
        out["intrinsics"] = collect(c.intrinsics)
    end
    if c.has_extrinsics != 0
        out["rotation"] = (c.rotation_w, c.rotation_x, c.rotation_y, c.rotation_z)
        out["position"] = (c.position_x, c.position_y, c.position_z)
        out["extrinsics"] = collect(c.extrinsics)
        out["projection"] = collect(c.projection)
    end
    ts = unsafe_string(pointer(collect(c.timestamp)))
    isempty(ts) || (out["timestamp"] = ts)
    out
end

# ---------------------------------------------------------------------------
# Backends
# ---------------------------------------------------------------------------

"""
    backends() -> Vector{String}

Registered decoding backends (comma-separated on the C side).
"""
function backends()
    s = ccall(_sym(:ptiff_backend_names), Ptr{Cchar}, ())
    s == C_NULL && return String[]
    str = unsafe_string(s)
    ccall(_sym(:ptiff_free_string), Cvoid, (Ptr{Cchar},), s)
    filter(!isempty, split(str, ','))
end

# ---------------------------------------------------------------------------
# Version
# ---------------------------------------------------------------------------

"""
    runtime_version() -> Version

Version of the loaded library.
"""
function runtime_version()
    maj = Ref{Int32}(0); min = Ref{Int32}(0); pat = Ref{Int32}(0)
    ccall(_sym(:ptiff_runtime_version_out), Cvoid,
            (Ref{Int32}, Ref{Int32}, Ref{Int32}), maj, min, pat)
    Version(Int(maj[]), Int(min[]), Int(pat[]))
end

"""
    compile_time_version() -> Version

Version the library compiled/linked against.
"""
function compile_time_version()
    maj = Ref{Int32}(0); min = Ref{Int32}(0); pat = Ref{Int32}(0)
    ccall(_sym(:ptiff_compile_time_version_out), Cvoid,
            (Ref{Int32}, Ref{Int32}, Ref{Int32}), maj, min, pat)
    Version(Int(maj[]), Int(min[]), Int(pat[]))
end

"""
    version() -> Version

Alias for [`runtime_version`].
"""
version() = runtime_version()

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

"""
    logger_set_level(level::LogLevel)

Set the process-wide logger level.
"""
logger_set_level(level::LogLevel) =
    ccall(_sym(:ptiff_logger_set_level), Cvoid, (Int32,), Int32(level))

"""
    logger_level() -> LogLevel

Current logger level.
"""
logger_level() = ccall(_sym(:ptiff_logger_level), Int32, ())

"""
    logger_log(level::LogLevel, message)

Emit a log record at `level`.
"""
logger_log(level::LogLevel, message::AbstractString) =
    ccall(_sym(:ptiff_logger_log), Cvoid, (Int32, Cstring), Int32(level), message)

# ---------------------------------------------------------------------------
# Internal: descriptor conversion + camera struct
# ---------------------------------------------------------------------------

_from_c(d::CImageDescriptor) = ImageDescriptor(
    Int(d.width), Int(d.height), PixelType(d.pixel_type), Int(d.channel_count),
    d.has_gsd != 0 ? d.gsd : nothing,
    d.has_tile_info != 0 ? Int(d.tile_width) : nothing,
    d.has_tile_info != 0 ? Int(d.tile_height) : nothing,
    Compression(d.compression))

end # module Ptiff
