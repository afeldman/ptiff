function test_version()
%TEST_VERSION  Version surface over the raw SWIG-generated Octave binding.
%
%   Pins the runtime/compile-time version returned both as a ptiff_version
%   struct (ptiff_runtime_version / ptiff_compile_time_version) and as three
%   out-params (ptiff_runtime_version_out / ptiff_compile_time_version_out).
%   Mirrors bindings/python/test/test_version.py and
%   bindings/ruby/test/test_version.rb.

r = ptiff_runtime_version();
c = ptiff_compile_time_version();
assert_ptiff(isequal([r.major r.minor r.patch], [c.major c.minor c.patch]), ...
  'runtime version must match compile-time version (struct)');

[r1, r2, r3] = ptiff_runtime_version_out();
[c1, c2, c3] = ptiff_compile_time_version_out();
assert_ptiff(isequal([r1 r2 r3], [c1 c2 c3]), ...
  'runtime version must match compile-time version (out-params)');
assert_ptiff(isequal([r1 r2 r3], [r.major r.minor r.patch]), ...
  'version out-params must match the struct fields');

fprintf('test_version: OK\n');
end
