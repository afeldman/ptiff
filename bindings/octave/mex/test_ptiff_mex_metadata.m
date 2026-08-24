function test_ptiff_mex_metadata()
%TEST_PTIFF_MEX_METADATA  Flattened ptiff.<domain>.<name> field read.
%
%   Verifies ptiff_metadata over the frozen interop fixture returns the same
%   ptiff.camera.* / ptiff.spice.* fields the other language bindings expect,
%   and that the camera.model / spice.frame values match. Mirrors the
%   cross-language PDS-layer assertions on the ptiff-octave path.

p = test_ptiff_fixture_path();
m = ptiff_metadata(p);
assert(iscell(m.keys) && iscell(m.values), 'keys/values cell arrays');
assert(numel(m.keys) == numel(m.values), 'parallel arrays');

% The fixture carries the cross-language ptiff.* fields.
idx = find(strcmp(m.keys, 'ptiff.camera.model'), 1);
assert(~isempty(idx), 'camera.model present');
assert(strcmp(m.values{idx}, 'pinhole'), 'camera.model == pinhole');

idx = find(strcmp(m.keys, 'ptiff.spice.frame'), 1);
assert(~isempty(idx), 'spice.frame present');
assert(strcmp(m.values{idx}, 'IAU_MOON'), 'spice.frame == IAU_MOON');

fprintf('test_ptiff_mex_metadata: OK (%d fields)\n', numel(m.keys));
end
