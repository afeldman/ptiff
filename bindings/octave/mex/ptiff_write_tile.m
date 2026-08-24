function ptiff_write_tile(handle, column, row, data)
%PTIFF_WRITE_TILE  Write one tile's raw bytes (uint8 vector) to an open sink.
ptiff_octave('write_tile', handle, column, row, data);
end
