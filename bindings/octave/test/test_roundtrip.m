function test_roundtrip()
%TEST_ROUNDTRIP  Round-trip test for the SWIG-generated ptiff Octave binding.
%
%   Writes a 32x32 UInt8 TIFF with 4x16x16 tiles through the ptiff_sink_*
%   surface and reads it back through ptiff_source_*, mirroring the Python,
%   Ruby and Go round-trips. Drives the raw SWIG-generated Octave API
%   directly (ptiff_sink_create / write_tile / close, ptiff_source_open /
%   read_tile / descriptor).
%
%   Run `make octave` in bindings/swig/ first (builds octave/lib/ptiff.oct).

c = ptiff_constants();

OUT = fullfile(tempdir(), 'roundtrip_octave.tif');
if exist(OUT, 'file'), delete(OUT); end

% ---- Build the image descriptor from the SWIG-generated class ----
desc = new_ptiff_image_descriptor();
desc.width = 32;
desc.height = 32;
desc.pixel_type = c.PTIFF_PIXEL_UINT8;
desc.channel_count = 1;
desc.has_tile_info = 1;
desc.tile_info.tile_width = 16;
desc.tile_info.tile_height = 16;
desc.has_compression = 0;

% ---- Write side (ptiff_sink_*) ----
sink = ptiff_sink_create(OUT, desc);
assert_ptiff(~isempty(sink), 'ptiff_sink_create returned NULL');

cols = ptiff_sink_tile_columns(sink);
rows = ptiff_sink_tile_rows(sink);
bs = ptiff_sink_tile_byte_size(sink);
assert_ptiff(cols == 2 && rows == 2, 'expected 2x2 tile grid');
assert_ptiff(bs == 16 * 16, 'expected tile byte size 256');

for cidx = 0:cols-1
  for ridx = 0:rows-1
    pattern = uint8(mod((cidx * rows + ridx), 256) * ones(1, bs));
    rc = ptiff_sink_write_tile(sink, cidx, ridx, pattern);
    assert_ptiff(rc == 0, sprintf('write_tile(%d,%d) rc=%d', cidx, ridx, rc));
  end
end
ptiff_sink_close(sink);
assert_ptiff(exist(OUT, 'file') == 2, 'no output file written');

% ---- Read side (ptiff_source_*) ----
% err_out is an int* OUTPUT -> surfaced as a second return value. It is only
% meaningful on failure (uninitialized on success), so we only assert the handle.
[src, ~] = ptiff_source_open(OUT);
assert_ptiff(~isempty(src), 'ptiff_source_open returned NULL');

rd = new_ptiff_image_descriptor();
rc = ptiff_source_descriptor(src, rd);
assert_ptiff(rc == 0, 'source_descriptor failed');
assert_ptiff(rd.width == 32 && rd.height == 32, 'desc size mismatch');
assert_ptiff(rd.tile_info.tile_width == 16 && rd.tile_info.tile_height == 16, ...
  'desc tile mismatch');

rbs = ptiff_source_tile_byte_size(src);
assert_ptiff(rbs == bs, 'source tile byte size mismatch');

% read tile (1,0): returns [rc, data, bytes_read]; data is a uint8 array.
[rc2, data, nread] = ptiff_source_read_tile(src, 1, 0, uint8(zeros(1, rbs)));
assert_ptiff(rc2 == 0, 'read_tile(1,0) failed');
assert_ptiff(nread == rbs, sprintf('bytes_read=%d, want %d', nread, rbs));
assert_ptiff(numel(data) == rbs, 'read data wrong length');
expected = uint8(mod((1 * rows + 0), 256) * ones(1, rbs));
assert_ptiff(isequal(data(:)', expected), 'read-back tile (1,0) mismatch');

ptiff_source_close(src);
delete_ptiff_image_descriptor(rd);
delete_ptiff_image_descriptor(desc);
delete(OUT);

fprintf('test_roundtrip: OK\n');
end
