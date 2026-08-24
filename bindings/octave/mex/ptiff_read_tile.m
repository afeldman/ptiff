function data = ptiff_read_tile(handle, column, row)
%PTIFF_READ_TILE  Read one decoded tile -> uint8 row vector.
%
%   data = ptiff_read_tile(handle, column, row) reads the (column,row) tile of
%   the open source. `data` is a uint8 row vector of exactly
%   ptiff_source_info(handle).tile_byte_size bytes.
data = ptiff_octave('read_tile', handle, column, row);
end
