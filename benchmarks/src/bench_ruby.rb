#!/usr/bin/env ruby
# frozen_string_literal: true

# Benchmark read/write throughput of the SWIG Ruby binding (Ptiff).
#
# Mirrors benchmarks/src/bench_python.py with the identical metric set and the
# same median-of-repeats timing methodology, over the raw SWIG-generated Ruby
# API (see bindings/ruby/test/test_roundtrip.rb for the API shape):
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
# Usage:
#   RUBYLIB=bindings/ruby/lib ruby benchmarks/src/bench_ruby.rb \
#       --out benchmarks/benchmark-results/ruby.json

require "json"
require "optparse"
require "fileutils"
require "tempfile"
require "timeout"
require "ptiff"

FIXTURES = File.expand_path("../fixtures", __dir__)
REPEATS = (ENV["BENCH_REPEATS"] || "20").to_i
ITERS = (ENV["BENCH_ITERS"] || "50").to_i
NAC_ITERS = [1, (ENV["BENCH_NAC_ITERS"] || "2").to_i].max

WRITE_SIZE = 128
WRITE_TILE = 64

def monotonic_ms
  Process.clock_gettime(Process::CLOCK_MONOTONIC) * 1000.0
end

def median_times
  times = []
  REPEATS.times do
    t0 = monotonic_ms
    yield
    times << (monotonic_ms - t0)
  end
  times.sort!
  { "repeats" => REPEATS, "median_ms" => times[times.size / 2].round(4),
    "min_ms" => times.first.round(4), "max_ms" => times.last.round(4) }
end

def new_desc(width, height, pixel_type, tile)
  d = Ptiff::Ptiff_image_descriptor.new
  d.width = width
  d.height = height
  d.pixel_type = pixel_type
  d.channel_count = 1
  d.has_tile_info = 1
  d.tile_info.tile_width = tile
  d.tile_info.tile_height = tile
  d.has_compression = 0
  d
end

def write_all_tiles(out_path, iters)
  d = new_desc(WRITE_SIZE, WRITE_SIZE, 0, WRITE_TILE)
  File.delete(out_path) if File.exist?(out_path)
  sink = Ptiff::ptiff_sink_create(out_path, d)
  cols = Ptiff::ptiff_sink_tile_columns(sink)
  rows = Ptiff::ptiff_sink_tile_rows(sink)
  bs = Ptiff::ptiff_sink_tile_byte_size(sink)
  pat = ([7].pack("C") * bs).b
  iters.times do
    (0...cols).each do |c|
      (0...rows).each do |r|
        rc = Ptiff::ptiff_sink_write_tile(sink, c, r, pat)
        raise "write_tile rc=#{rc}" unless rc.zero?
      end
    end
  end
  Ptiff::ptiff_sink_close(sink)
end

def read_all_tiles(path, iters)
  src = Ptiff::ptiff_source_open(path)[0]
  ncols = Ptiff::ptiff_source_tile_columns(src)
  nrows = Ptiff::ptiff_source_tile_rows(src)
  rbs = Ptiff::ptiff_source_tile_byte_size(src)
  iters.times do
    buf = "\0".b * rbs
    (0...ncols).each do |c|
      (0...nrows).each do |r|
        Ptiff::ptiff_source_read_tile(src, c, r, buf)
      end
    end
  end
  Ptiff::ptiff_source_close(src)
end

def fixture_info(name)
  p = File.join(FIXTURES, name)
  { "file" => p, "size_bytes" => File.size(p), "exists" => File.exist?(p) }
end

options = {}
OptionParser.new do |opts|
  opts.on("--out PATH", "output JSON path") { |v| options[:out] = v }
  opts.on("--real", "also read fixtures/real_lola_512.tif (real NASA LOLA crop) if present") do
    options[:real] = true
  end
  opts.on("--nac", "also read fixtures/nac_dtm.tif (real NASA LRO-NAC DTM) if present") do
    options[:nac] = true
  end
end.parse!

# warm-up: load module + libptiff backends once
new_desc(1, 1, 0, 1)

real_tif = File.join(FIXTURES, "real_lola_512.tif")
nac_tif = File.join(FIXTURES, "nac_dtm.tif")

tmpdir = Dir.mktmpdir
begin
  write_all_tiles(File.join(tmpdir, "write_small.tif"), 1) # self-check write works

  metrics = {
    "write_all_tiles_ms" => median_times { write_all_tiles(File.join(tmpdir, "write_small.tif"), ITERS) },
    "read_uint8_128_ms"  => median_times { read_all_tiles(File.join(FIXTURES, "uint8_128.tif"), ITERS) },
    "read_uint8_512_ms"  => median_times { read_all_tiles(File.join(FIXTURES, "uint8_512.tif"), ITERS) },
    "read_f32_512_ms"    => median_times { read_all_tiles(File.join(FIXTURES, "f32_512.tif"), ITERS) },
  }

  v = Ptiff::ptiff_runtime_version
  doc = {
    "language" => "ruby",
    "binding_version" => "#{v.major}.#{v.minor}.#{v.patch}",
    "repeats" => REPEATS,
    "iterations_per_sample" => ITERS,
    "write_image" => { "width" => WRITE_SIZE, "height" => WRITE_SIZE,
                       "pixel_type" => "uint8", "tile" => WRITE_TILE },
    "fixtures" => { "uint8_128" => fixture_info("uint8_128.tif"),
                    "uint8_512" => fixture_info("uint8_512.tif"),
                    "f32_512"   => fixture_info("f32_512.tif") },
    "metrics" => metrics,
  }

  # --real: read a genuine NASA LOLA elevation crop (copied into benchmarks/
  # fixtures by run_benchmarks.sh --real) in addition to the synthetic fixtures.
  if options[:real] && File.exist?(real_tif)
    metrics["read_real_lola_512_ms"] = median_times do
      read_all_tiles(real_tif, ITERS)
    end
    doc["fixtures"]["real_lola_512"] = fixture_info("real_lola_512.tif")
  end

  # --nac: read the full real NASA LRO-NAC DTM (616 tiles/pass, ~55 MB). Each
  # inner sample is a full pass over every tile; NAC_ITERS keeps it bounded.
  if options[:nac] && File.exist?(nac_tif)
    metrics["read_nac_ms"] = median_times do
      read_all_tiles(nac_tif, NAC_ITERS)
    end
    doc["fixtures"]["nac_dtm"] = fixture_info("nac_dtm.tif")
    doc["nac_ifd"] = { "tiles_per_pass" => 616, "nac_iters" => NAC_ITERS }
  end

  out = options[:out]
  Dir.mkdir(File.dirname(out)) unless Dir.exist?(File.dirname(out))
  File.write(out, JSON.pretty_generate(doc) + "\n")
  puts JSON.generate("language" => "ruby", "metrics" => metrics)
ensure
  FileUtils.remove_entry(tmpdir) if tmpdir
end
