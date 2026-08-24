function ptiff_sink_close(handle)
%PTIFF_SINK_CLOSE  Flush and close a sink handle. Only now is the file valid.
ptiff_octave('sink_close', handle);
end
