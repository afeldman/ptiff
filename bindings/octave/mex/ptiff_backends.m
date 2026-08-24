function names = ptiff_backends()
%PTIFF_BACKENDS  Comma-separated list of registered decoding backends.
names = ptiff_octave('backend_names');
end
