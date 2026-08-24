function ptiff_close(handle)
%PTIFF_CLOSE  Close a source handle returned by ptiff_open. Idempotent.
ptiff_octave('source_close', handle);
end
