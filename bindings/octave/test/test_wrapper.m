function test_wrapper()
%TEST_WRAPPER  Idiomatic Octave classdef wrappers (Image/Camera/Tile/Logger/Metadata).
%
%   Mirrors the Python src/ptiff/ wrapper tests: a small object layer over the
%   SWIG low-level module. The classes live in bindings/octave/lib/.

c = ptiff_constants();

% ---- Metadata over the interop fixture ----
fixture = fullfile(fileparts(mfilename('fullpath')), '..', '..', '..', ...
    'scripts', 'samples', 'ptiff_interop_fixture.tif');
m = Metadata(fixture);
assert_ptiff(m.width == 128 && m.height == 128, 'metadata size 128x128');
assert_ptiff(m.channel_count == 1, 'metadata channel_count');
assert_ptiff(strcmp(m.pixel_type_name(), 'UInt8'), 'metadata pixel name');
assert_ptiff(m.tile_width == 128 && m.tile_height == 128, 'metadata tile size');
assert_ptiff(strcmp(m.field('ptiff.camera.model'), 'pinhole'), 'metadata camera.model');
assert_ptiff(strcmp(m.field('ptiff.spice.frame'), 'IAU_MOON'), 'metadata spice.frame');

% ---- Camera.from_path over the fixture ----
cam = Camera.from_path(fixture);
assert_ptiff(cam.has_intrinsics(), 'fixture has intrinsics');
assert_ptiff(abs(cam.focal_length_x() - 700.0) < 1e-9, 'fx=700');
assert_ptiff(abs(cam.principal_x() - 64.0) < 1e-9, 'cx=64');
assert_ptiff(numel(cam.intrinsics_matrix()) == 9, 'intrinsics size');
assert_ptiff(numel(cam.projection_matrix()) == 12, 'projection size');

% ---- Image.open read path over the fixture ----
img = Image.open(fixture);
assert_ptiff(img.width() == 128 && img.height() == 128, 'open size');
assert_ptiff(img.tile_byte_size() == 16384, 'open tile byte size');
t = img.read_tile(0, 0);
assert_ptiff(t.column == 0 && t.row == 0, 'tile position');
assert_ptiff(numel(t.data) == 16384, 'tile data length');
img.close();

% ---- Image.create / write / read roundtrip ----
out = tempname();
img2 = Image.create(out, 32, 32, 'pixel_type', c.PTIFF_PIXEL_UINT8, ...
    'tile_width', 16, 'tile_height', 16);
assert_ptiff(img2.tile_columns() == 2 && img2.tile_rows() == 2, 'create grid');
bs = img2.tile_byte_size();
assert_ptiff(bs == 256, 'create tile byte size');
img2.write_tile(0, 0, uint8(7 * ones(1, bs)));
img2.write_tile(1, 1, uint8(9 * ones(1, bs)));
img2.close();

rd = Image.open(out);
assert_ptiff(rd.read_tile(0, 0).data(1) == 7, 'readback tile00');
assert_ptiff(rd.read_tile(1, 1).data(1) == 9, 'readback tile11');
rd.close();

% ---- Image.create with a camera roundtrip ----
wc = Camera('focal_length_x', 700.0, 'focal_length_y', 715.0, ...
    'principal_x', 32.0, 'principal_y', 24.0, 'rotation_w', 1.0, ...
    'position_x', 1.0, 'position_y', 2.0, 'position_z', 3.0, ...
    'timestamp', '2026-08-21T12:34:56.000Z');
cout = tempname();
cimg = Image.create(cout, 32, 32, 'pixel_type', c.PTIFF_PIXEL_UINT8, ...
    'tile_width', 16, 'tile_height', 16, 'camera', wc);
cimg.write_tile(0, 0, uint8(zeros(1, cimg.tile_byte_size())));
cimg.close();
crd = Image.open(cout);
got = crd.camera();
assert_ptiff(abs(got.focal_length_x() - 700.0) < 1e-9, 'camera fx');
assert_ptiff(abs(got.focal_length_y() - 715.0) < 1e-9, 'camera fy');
assert_ptiff(strcmp(got.timestamp(), '2026-08-21T12:34:56.000Z'), 'camera timestamp');
P = got.projection_matrix();
assert_ptiff(abs(P(4) - (700 * 1 + 32 * 3)) < 1e-6, 'camera P[0,3]');
assert_ptiff(abs(P(8) - (715 * 2 + 24 * 3)) < 1e-6, 'camera P[1,3]');
assert_ptiff(abs(P(12) - 3.0) < 1e-6, 'camera P[2,3]');
crd.close();

% ---- Logger wrapper ----
lg = Logger();
orig = lg.get_level();
lg.set_level(lg.ERROR);
assert_ptiff(lg.get_level() == lg.ERROR, 'logger set level');
lg.debug('filtered out');
lg.error('emitted');
lg.set_level(orig);

delete (out); delete (cout);
fprintf('test_wrapper: OK\n');
end
