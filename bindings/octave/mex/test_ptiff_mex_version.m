function test_ptiff_mex_version()
%TEST_PTIFF_MEX_VERSION  Version / ABI / backend surface over the MEX adapter.
%
%   Verifies the compile/runtime version struct, the monotone ABI break
%   counter, and the backend registry name. Mirrors the SWIG test_version.m /
%   test_backend.m coverage against the C ABI, but over ptiff_octave.

v = ptiff_version();
assert(v.compile_major == v.runtime_major, 'major');
assert(v.compile_minor == v.runtime_minor, 'minor');
assert(v.compile_patch == v.runtime_patch, 'patch');
assert(v.compile_major >= 1, 'major >= 1');

assert(ptiff_abi_version() >= 1, 'abi_version >= 1');

names = ptiff_backends();
assert(ischar(names) && ~isempty(names), 'backend names string');

fprintf('test_ptiff_mex_version: OK (%d.%d.%d, abi=%d, backends=%s)\n', ...
  v.compile_major, v.compile_minor, v.compile_patch, ptiff_abi_version(), names);
end
