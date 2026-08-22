function d = tile_descriptor_ptiff()
%TILE_DESCRIPTOR_PTIFF  A 32x32 UInt8, 16x16-tiled descriptor (2x2 grid).
%
%   Returns a heap-allocated ptiff_image_descriptor (release with
%   delete_ptiff_image_descriptor).
c = ptiff_constants();
d = new_ptiff_image_descriptor();
d.width = 32;
d.height = 32;
d.pixel_type = c.PTIFF_PIXEL_UINT8;
d.channel_count = 1;
d.has_tile_info = 1;
d.tile_info.tile_width = 16;
d.tile_info.tile_height = 16;
d.has_compression = 0;
end
