function test_ptiff_mex_info()
%TEST_PTIFF_MEX_INFO  ptiff_info descriptor read on a path (no live handle).
%
%   Verifies the header-only descriptor read (`ptiff_info`) over the frozen
%   interop fixture: size, pixel type, channel count, tile geometry and
%   compression. Mirrors the SWIG test_image.m descriptor assertions.

p = test_ptiff_fixture_path();
c = ptiff_constants_mex();
d = ptiff_info(p);
assert(d.width == 128 && d.height == 128, 'size 128x128');
assert(d.channel_count == 1, 'channel_count');
assert(d.pixel_type == c.PTIFF_PIXEL_UINT8, 'pixel_type uint8');
assert(d.has_tile_info == 1, 'has tiles');
assert(d.tile_width == 128 && d.tile_height == 128, 'tile size');
assert(d.has_compression == 0 || d.compression == c.PTIFF_COMPRESSION_NONE, ...
  'uncompressed fixture');
fprintf('test_ptiff_mex_info: OK\n');
end
