function h = ptiff_create(path, width, height, pixel_type, channels, tile_width, tile_height, camera)
%PTIFF_CREATE  Create a tiled PTIFF for writing -> uint64 handle.
%
%   h = ptiff_create(path, w, h, pixel_type, channels, tile_w, tile_h)
%   opens a new tiled image. Write every tile with ptiff_write_tile, then
%   ptiff_sink_close(h) to flush the file (only then is it valid to read).
%
%   Optionally pass a camera struct (with has_intrinsics/has_extrinsics,
%   focal_length_*, principal_*, rotation_*, position_*, timestamp fields, as
%   returned by the `camera` command) as the 8th argument, or [] for none, to
%   persist structured camera metadata.
if nargin < 8
  h = ptiff_octave('create', path, width, height, pixel_type, channels, ...
      tile_width, tile_height);
else
  h = ptiff_octave('create', path, width, height, pixel_type, channels, ...
      tile_width, tile_height, camera);
end
end
