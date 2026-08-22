function test_image()
%TEST_IMAGE  Image descriptor surface over the raw SWIG-generated binding.
%
%   Ports bindings/ruby/test/test_image.rb onto ptiff_image_descriptor +
%   ptiff_image_create. Requires the double*/int* OUTPUT typemaps added to
%   typemaps.i for ptiff_image_gsd/ptiff_image_compression
%   (ptiff_image_tile_info already works without one: its out-param is a
%   SWIG-wrapped struct pointer, not a bare primitive).

c = ptiff_constants();

% ---- Round-trip: create an image from a fully-specified descriptor ----
d = new_ptiff_image_descriptor();
d.width = 10;
d.height = 20;
d.pixel_type = c.PTIFF_PIXEL_UINT16;
d.channel_count = 3;
d.has_gsd = 1;
d.gsd = 0.5;
d.has_tile_info = 1;
d.tile_info.tile_width = 16;
d.tile_info.tile_height = 16;
d.has_compression = 1;
d.compression = c.PTIFF_COMPRESSION_DEFLATE;

img = ptiff_image_create(d);
assert_ptiff(~isempty(img), 'image_create returned empty');
assert_ptiff(ptiff_image_width(img) == 10, 'width');
assert_ptiff(ptiff_image_height(img) == 20, 'height');
assert_ptiff(ptiff_image_pixel_type(img) == c.PTIFF_PIXEL_UINT16, 'pixel_type');
assert_ptiff(ptiff_image_channel_count(img) == 3, 'channel_count');

[ok, gsd] = ptiff_image_gsd(img);
assert_ptiff(ok ~= 0, 'gsd present');
assert_ptiff(abs(gsd - 0.5) < 1e-9, 'gsd value');

ti = new_ptiff_tile_info();
ok2 = ptiff_image_tile_info(img, ti);
assert_ptiff(ok2 ~= 0, 'tile_info present');
assert_ptiff(ti.tile_width == 16 && ti.tile_height == 16, 'tile_info dims');
delete_ptiff_tile_info(ti);

[ok3, comp] = ptiff_image_compression(img);
assert_ptiff(ok3 ~= 0, 'compression present');
assert_ptiff(comp == c.PTIFF_COMPRESSION_DEFLATE, 'compression kind');

ptiff_image_destroy(img);
delete_ptiff_image_descriptor(d);

% ---- Optionals absent: gsd / tile_info / compression all absent ----
d2 = new_ptiff_image_descriptor();
d2.width = 4;
d2.height = 4;
d2.pixel_type = c.PTIFF_PIXEL_FLOAT32;
d2.channel_count = 1;

img2 = ptiff_image_create(d2);
assert_ptiff(~isempty(img2), 'image_create(absent) returned empty');
[okA, ~] = ptiff_image_gsd(img2);
assert_ptiff(okA == 0, 'gsd absent');

ti2 = new_ptiff_tile_info();
okB = ptiff_image_tile_info(img2, ti2);
assert_ptiff(okB == 0, 'tile_info absent');
delete_ptiff_tile_info(ti2);

[okC, ~] = ptiff_image_compression(img2);
assert_ptiff(okC == 0, 'compression absent');

ptiff_image_destroy(img2);
delete_ptiff_image_descriptor(d2);

fprintf('test_image: OK\n');
end
