function test_ptiff_mex_camera()
%TEST_PTIFF_MEX_CAMERA  Structured camera read + write over the MEX adapter.
%
%   Reads the intrinsics-only camera from the frozen interop fixture, then
%   writes a full camera (intrinsics + extrinsics -> derived projection) via
%   the optional `camera` argument to ptiff_create and reads it back. Mirrors
%   the SWIG test_wrapper.m Camera.from_path / Image.create(camera=...) flow
%   on the ptiff-octave path.

p = test_ptiff_fixture_path();
c = ptiff_constants_mex();

% ---- Read: the fixture is intrinsics-only (no extrinsics/projection) ----
cam = ptiff_octave('camera', p);
assert(cam.has_intrinsics == 1, 'fixture has intrinsics');
assert(cam.has_extrinsics == 0, 'fixture intrinsics-only');
assert(abs(cam.focal_length_x - 700.0) < 1e-9, 'fx=700');
assert(abs(cam.principal_x - 64.0) < 1e-9, 'cx=64');
assert(numel(cam.intrinsics) == 9, 'intrinsics matrix size');

% ---- Write a full camera, read it back with derived matrices ----
wc = struct( ...
  'has_intrinsics', 1, 'has_extrinsics', 1, ...
  'focal_length_x', 700.0, 'focal_length_y', 715.0, ...
  'principal_x', 32.0, 'principal_y', 24.0, ...
  'rotation_w', 1.0, 'rotation_x', 0, 'rotation_y', 0, 'rotation_z', 0, ...
  'position_x', 1.0, 'position_y', 2.0, 'position_z', 3.0, ...
  'timestamp', '2026-08-21T12:34:56.000Z');

cout = tempname();
try
  h = ptiff_create(cout, 32, 32, c.PTIFF_PIXEL_UINT8, 1, 16, 16, wc);
  bs = ptiff_sink_info(h).tile_byte_size;
  ptiff_write_tile(h, 0, 0, uint8(zeros(1, bs)));
  ptiff_sink_close(h);

  got = ptiff_octave('camera', cout);
  assert(got.has_intrinsics == 1 && got.has_extrinsics == 1, 'write both groups');
  assert(abs(got.focal_length_x - 700.0) < 1e-9, 'fx roundtrip');
  assert(abs(got.focal_length_y - 715.0) < 1e-9, 'fy roundtrip');
  assert(strcmp(got.timestamp, '2026-08-21T12:34:56.000Z'), 'timestamp roundtrip');
  assert(numel(got.intrinsics) == 9 && numel(got.extrinsics) == 12, 'K + [R|t]');
  assert(numel(got.projection) == 12, 'derived projection size');
  fprintf('test_ptiff_mex_camera: OK (fx=%g, fy=%g)\n', ...
    got.focal_length_x, got.focal_length_y);
catch err
  ptiff_octave('clear');
  rethrow(err);
end
delete(cout);
end
