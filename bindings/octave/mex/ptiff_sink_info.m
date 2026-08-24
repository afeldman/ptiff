function s = ptiff_sink_info(handle)
%PTIFF_SINK_INFO  Tile grid size of an open sink handle.
s = ptiff_octave('sink_info', handle);
end
