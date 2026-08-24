function test_ptiff_mex_roundtrip()
%TEST_PTIFF_MEX_ROUNDTRIP  Sink create / write tile / read-back roundtrip.
%
%   Writes a small tiled image with the MEX sink (ptiff_create,
%   ptiff_write_tile, ptiff_sink_close) then reads it back with the MEX
%   source and checks the pixel payload. Mirrors the SWIG test_roundtrip.m
%   coverage but stays entirely on the ptiff-octave path.

c = ptiff_constants_mex();
out = tempname();
try
  h = ptiff_create(out, 32, 32, c.PTIFF_PIXEL_UINT8, 1, 16, 16);
  s = ptiff_sink_info(h);
  assert(s.tile_columns == 2 && s.tile_rows == 2, 'tile grid 2x2');
  bs = s.tile_byte_size;
  assert(bs == 256, 'tile byte size 256');

  ptiff_write_tile(h, 0, 0, uint8(7 * ones(1, bs)));
  ptiff_write_tile(h, 1, 1, uint8(9 * ones(1, bs)));
  ptiff_sink_close(h);

  r = ptiff_open(out);
  d = ptiff_read_tile(r, 0, 0); assert(d(1) == 7, 'readback tile(0,0)');
  d = ptiff_read_tile(r, 1, 1); assert(d(1) == 9, 'readback tile(1,1)');
  ptiff_close(r);
  fprintf('test_ptiff_mex_roundtrip: OK\n');
catch err
  ptiff_octave('clear');
  rethrow(err);
end
delete(out);
end
