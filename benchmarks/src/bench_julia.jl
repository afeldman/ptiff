#!/usr/bin/env julia
# Benchmark read/write throughput of the Ptiff.jl Julia binding.
#
# Mirrors benchmarks/src/bench_python.py with the identical metric set and the
# same median-of-repeats timing methodology, over the public Ptiff.jl API
# (see bindings/julia/test/runtests.jl for the API shape):
#
#   * write_all_tiles  -- create a 128x128 UInt8 tiled TIFF, write every tile
#   * read_uint8_128   -- open benchmarks/fixtures/uint8_128.tif, read all tiles
#   * read_uint8_512   -- open benchmarks/fixtures/uint8_512.tif, read all tiles
#   * read_f32_512     -- open benchmarks/fixtures/f32_512.tif, read all tiles
#
# Each metric repeats an inner body `iters` times (default 50) to amortize
# noise; the per-language JSON is written to --out for run_benchmarks.sh /
# summary.py.
#
# Usage (from benchmarks/):
#   julia src/bench_julia.jl --out benchmark-results/julia.json
#   julia src/bench_julia.jl --out benchmark-results/julia.json --real --nac

using Libdl

# Load the Ptiff.jl module from bindings/julia (the benchmark harness keeps the
# workspace layout, so the module's default libpath() resolves target/release).
include(joinpath(@__DIR__, "..", "..", "bindings", "julia", "src", "Ptiff.jl"))
using .Ptiff

const FIXTURES = abspath(joinpath(@__DIR__, "..", "fixtures"))
const REPEATS = parse(Int, get(ENV, "BENCH_REPEATS", "20"))
const ITERS = parse(Int, get(ENV, "BENCH_ITERS", "50"))
const NAC_ITERS = max(1, parse(Int, get(ENV, "BENCH_NAC_ITERS", "2")))
const WRITE_SIZE = 128
const WRITE_TILE = 64

# ---------------------------------------------------------------------------
# CLI arg parsing (--out PATH, --real, --nac)
# ---------------------------------------------------------------------------
function parse_args(args)
    opts = Dict{String,Any}("out" => "", "real" => false, "nac" => false)
    i = 1
    while i <= length(args)
        a = args[i]
        if a == "--out"
            i += 1
            opts["out"] = args[i]
        elseif a == "--real"
            opts["real"] = true
        elseif a == "--nac"
            opts["nac"] = true
        end
        i += 1
    end
    return opts
end
opts = parse_args(ARGS)
out_path = opts["out"]
real = opts["real"]
nac = opts["nac"]

# ---------------------------------------------------------------------------
# Timing helpers
# ---------------------------------------------------------------------------
function monotonic_ms()
    t = time_ns()
    return t / 1e6
end

function median_times(f)
    times = Float64[]
    for _ in 1:REPEATS
        t0 = monotonic_ms()
        f()
        push!(times, monotonic_ms() - t0)
    end
    sort!(times)
    mid = div(length(times), 2) + 1
    return Dict{String,Any}(
        "repeats" => REPEATS,
        "median_ms" => round(times[mid], digits=4),
        "min_ms" => round(times[1], digits=4),
        "max_ms" => round(times[end], digits=4),
    )
end

# ---------------------------------------------------------------------------
# Benchmark bodies
# ---------------------------------------------------------------------------

# grid dimensions for a tiled fixture from its descriptor
function tile_grid(path)
    info = Ptiff.read_info(path)
    tw = info.tile_width::Int
    th = info.tile_height::Int
    cols = cld(info.width, tw)
    rows = cld(info.height, th)
    return (cols, rows)
end

function write_all_tiles(path)
    sink = Ptiff.create_sink(path, WRITE_SIZE, WRITE_SIZE, PTIFF_PIXEL_UINT8, 1;
                                tile_width=WRITE_TILE, tile_height=WRITE_TILE)
    cols, rows, tbs = Ptiff.sink_info(sink)
    pat = zeros(UInt8, tbs)
    for r in 0:(rows - 1)
        for c in 0:(cols - 1)
            Ptiff.write_tile(sink, c, r, pat)
        end
    end
    Ptiff.close_sink(sink)
end

function read_all_tiles(path, iters)
    src = Ptiff.open_source(path)
    cols, rows = tile_grid(path)
    for _ in 1:iters
        for r in 0:(rows - 1)
            for c in 0:(cols - 1)
                Ptiff.read_tile(src, c, r)
            end
        end
    end
    Ptiff.close_source(src)
end

function fixture_info(name)
    p = joinpath(FIXTURES, name)
    return Dict{String,Any}(
        "file" => p,
        "size_bytes" => filesize(p),
        "exists" => isfile(p),
    )
end

# ---------------------------------------------------------------------------
# Minimal deterministic JSON encoder (avoids depending on the JSON package)
# ---------------------------------------------------------------------------
function json_string(x)
    io = IOBuffer()
    _write_json(io, x)
    return String(take!(io))
end

function _write_json(io, x::AbstractString)
    print(io, '"', replace(x, "\"" => "\\\""), '"')
end

function _write_json(io, x::Integer)
    print(io, string(x))
end

function _write_json(io, x::AbstractFloat)
    print(io, string(x))
end

function _write_json(io, x::Bool)
    print(io, x ? "true" : "false")
end

function _write_json(io, x::Nothing)
    print(io, "null")
end

function _write_json(io, d::Dict)
    print(io, '{')
    first = true
    for (k, v) in d
        first || print(io, ',')
        first = false
        _write_json(io, string(k))
        print(io, ':')
        _write_json(io, v)
    end
    print(io, '}')
end

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

# warm-up: load module + libptiff backends once
Ptiff.read_info(joinpath(FIXTURES, "uint8_128.tif"))

real_tif = joinpath(FIXTURES, "real_lola_512.tif")
nac_tif = joinpath(FIXTURES, "nac_dtm.tif")

mktempdir() do tmpdir
    write_all_tiles(joinpath(tmpdir, "write_small.tif"))  # self-check write works

    metrics = Dict{String,Any}(
        "write_all_tiles_ms" => median_times(() -> write_all_tiles(joinpath(tmpdir, "write_small.tif"))),
        "read_uint8_128_ms"  => median_times(() -> read_all_tiles(joinpath(FIXTURES, "uint8_128.tif"), ITERS)),
        "read_uint8_512_ms"  => median_times(() -> read_all_tiles(joinpath(FIXTURES, "uint8_512.tif"), ITERS)),
        "read_f32_512_ms"    => median_times(() -> read_all_tiles(joinpath(FIXTURES, "f32_512.tif"), ITERS)),
    )

    v = Ptiff.runtime_version()
    doc = Dict{String,Any}(
        "language" => "julia",
        "binding_version" => "$(v.major).$(v.minor).$(v.patch)",
        "repeats" => REPEATS,
        "iterations_per_sample" => ITERS,
        "write_image" => Dict{String,Any}(
            "width" => WRITE_SIZE,
            "height" => WRITE_SIZE,
            "pixel_type" => "uint8",
            "tile" => WRITE_TILE,
        ),
        "fixtures" => Dict{String,Any}(
            "uint8_128" => fixture_info("uint8_128.tif"),
            "uint8_512" => fixture_info("uint8_512.tif"),
            "f32_512"   => fixture_info("f32_512.tif"),
        ),
        "metrics" => metrics,
    )

    if real && isfile(real_tif)
        metrics["read_real_lola_512_ms"] = median_times(() -> read_all_tiles(real_tif, ITERS))
        doc["fixtures"]["real_lola_512"] = fixture_info("real_lola_512.tif")
    end

    if nac && isfile(nac_tif)
        metrics["read_nac_ms"] = median_times(() -> read_all_tiles(nac_tif, NAC_ITERS))
        doc["fixtures"]["nac_dtm"] = fixture_info("nac_dtm.tif")
        doc["nac_ifd"] = Dict{String,Any}("tiles_per_pass" => 616, "nac_iters" => NAC_ITERS)
    end

    if out_path != ""
        mkpath(dirname(out_path))
        open(out_path, "w") do io
            # stable deterministic JSON (keys in insertion order)
            println(io, json_string(doc))
        end
    end
    println(json_string(Dict("language" => "julia", "metrics" => metrics)))
end
