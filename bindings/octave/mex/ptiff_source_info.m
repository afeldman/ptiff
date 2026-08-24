function s = ptiff_source_info(handle)
%PTIFF_SOURCE_INFO  Image + tile grid size of an open source handle.
s = ptiff_octave('source_info', handle);
end
