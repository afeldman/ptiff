function h = ptiff_open(path)
%PTIFF_OPEN  Open a PTIFF file for reading -> uint64 handle.
%
%   h = ptiff_open(path) opens the file over the Rust core via the C ABI and
%   returns an opaque uint64 handle. Release it with ptiff_close(h). It is
%   also closed automatically by ptiff_octave('clear').
%
%   Throws a ptiff:... error if the file cannot be opened.
h = ptiff_octave('open', path);
end
