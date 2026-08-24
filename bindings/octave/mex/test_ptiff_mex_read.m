function test_ptiff_mex_read()
%TEST_PTIFF_MEX_READ  Source open / descriptor / tile read over the MEX adapter.
%
%   Opens the frozen C++-Oracle interop fixture and checks the descriptor
%   (128x128, 1 channel, 16384-byte tile) and a full tile read. Mirrors the
%   write+read flow the SWIG binding exercised, but routing through
%   ptiff_open / ptiff_source_info / ptiff_read_tile.

p = test_ptiff_fixture_path();

h = ptiff_open(p);
s = ptiff_source_info(h);
assert(s.width == 128, 'width');
assert(s.height == 128, 'height');
assert(s.channel_count == 1, 'channel_count');
assert(s.pixel_type == ptiff_constants_mex().PTIFF_PIXEL_UINT8, 'pixel_type');
assert(s.tile_width == 128 && s.tile_height == 128, 'tile size');
assert(s.tile_byte_size == 16384, 'tile byte size');

t = ptiff_read_tile(h, 0, 0);
assert(numel(t) == 16384, 'tile data length');

ptiff_close(h);

fprintf('test_ptiff_mex_read: OK\n');
end
