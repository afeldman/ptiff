function m = ptiff_metadata(path)
%PTIFF_METADATA  Read all flattened ptiff.<domain>.<name> extension fields.
%
%   m = ptiff_metadata(path) returns a struct with `keys` and `values` cell
%   arrays (parallel). Example keys: ptiff.camera.model, ptiff.spice.frame.
m = ptiff_octave('fields', path);
end
