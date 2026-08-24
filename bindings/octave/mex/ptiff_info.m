function d = ptiff_info(path)
%PTIFF_INFO  Read an image's descriptor (size/type/tiles/compression) directly.
%
%   d = ptiff_info(path) reads the image header without retaining a handle.
%   Fields: width, height, pixel_type, channel_count, has_gsd, gsd,
%   has_tile_info, tile_width, tile_height, has_compression, compression.
d = ptiff_octave('open_info', path);
end
