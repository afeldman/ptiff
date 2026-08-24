#!/usr/bin/env ruby
# Round-trip test for the promoted SWIG-generated Ruby binding (bindings/ruby).
#
# Writes a 32x32 UInt8 TIFF with 4x16x16 tiles through the ptiff_sink_* surface
# and reads it back through ptiff_source_*, mirroring the Python and Go
# round-trips. It drives the raw SWIG-generated Ruby API directly
# (ptiff_sink_create/write_tile/close, ptiff_source_open/read_tile/
# descriptor) to demonstrate what SWIG emits for the libptiff C ABI.
#
# Run `make ruby` in bindings/swig/ (builds ruby/lib/ptiff.{bundle,so}) then:
#   RUBYLIB=../lib ruby test_roundtrip.rb
# or `bundle exec rake test` from bindings/ruby/ (Rakefile puts lib/ on the
# load path).
require 'ptiff'

OUT = File.join(__dir__, "roundtrip_ruby.tif")

# ---- Build the image descriptor from the SWIG-generated class ----
desc = Ptiff::Ptiff_image_descriptor.new
desc.width = 32
desc.height = 32
desc.pixel_type = 0            # PTIFF_PIXEL_UINT8
desc.channel_count = 1
desc.has_tile_info = 1
desc.tile_info.tile_width = 16
desc.tile_info.tile_height = 16
desc.has_compression = 0

# ---- Write side (ptiff_sink_*) ----
File.delete(OUT) if File.exist?(OUT)
sink = Ptiff::ptiff_sink_create(OUT, desc)
abort "ptiff_sink_create returned nil" unless sink

cols = Ptiff::ptiff_sink_tile_columns(sink)
rows = Ptiff::ptiff_sink_tile_rows(sink)
bs = Ptiff::ptiff_sink_tile_byte_size(sink)
puts "sink tile grid #{cols}x#{rows}, byte_size=#{bs}"
abort "grid #{cols}x#{rows}, want 2x2" unless cols == 2 && rows == 2
abort "byte_size #{bs}, want #{16 * 16}" unless bs == 16 * 16

(0...cols).each do |c|
  (0...rows).each do |r|
    pattern = ([((c * rows + r) % 256)].pack("C") * bs).freeze
    rc = Ptiff::ptiff_sink_write_tile(sink, c, r, pattern)
    abort "write_tile(#{c},#{r}) rc=#{rc}" unless rc == 0
  end
end

Ptiff::ptiff_sink_close(sink)
abort "no output file #{OUT}" unless File.exist?(OUT)
puts "written #{OUT}, size=#{File.size(OUT)}"

# ---- Read side (ptiff_source_*) ----
# err_out is an int* OUTPUT -> surfaced as a second return value.
res = Ptiff::ptiff_source_open(OUT)
src = res[0]
abort "source_open returned nil" unless src
puts "source_open: src=#{src}"

d = Ptiff::Ptiff_image_descriptor.new
rc = Ptiff::ptiff_source_descriptor(src, d)
abort "source_descriptor rc=#{rc}" unless rc == 0
puts "read-back descriptor: #{d.width}x#{d.height} px=#{d.pixel_type} " \
  "tile=#{d.tile_info.tile_width}x#{d.tile_info.tile_height} comp=#{d.compression}"
abort "desc mismatch" unless d.width == 32 && d.height == 32
abort "tile mismatch" unless d.tile_info.tile_width == 16 && d.tile_info.tile_height == 16

rcols = Ptiff::ptiff_source_tile_columns(src)
rrows = Ptiff::ptiff_source_tile_rows(src)
rbs = Ptiff::ptiff_source_tile_byte_size(src)
puts "source tile grid #{rcols}x#{rrows}, byte_size=#{rbs}"
abort "src grid mismatch" unless rcols == 2 && rrows == 2

# read tile (1,0) into a writable, non-frozen String of the right size
buf = "\0".b * rbs
res2 = Ptiff::ptiff_source_read_tile(src, 1, 0, buf)   # => [rc, bytes_read]
puts "read_tile(1,0): #{res2.inspect}"
abort "read_tile rc=#{res2[0]}" unless res2[0] == 0
expected = ([((1 * rrows + 0) % 256)].pack("C") * rbs).b
abort "read-back tile (1,0) mismatch" unless buf == expected

Ptiff::ptiff_source_close(src)

puts "ROUNDTRIP OK"
