function test_backend()
%TEST_BACKEND  Backend registry surface over the raw SWIG-generated binding.
%
%   ptiff_backend_names returns a single comma-space-joined, caller-owned C
%   string (documented as released via ptiff_free_string). SWIG's default
%   char* return typemap copies the C buffer into a language-owned value, so
%   what the test gets back is a plain Octave char (row) string; calling
%   ptiff_free_string on it is NOT safe (same abort bug as the Python/Ruby
%   ports) and is therefore never exercised here.

joined = ptiff_backend_names();
% May legitimately be empty when linking statically without whole-archive
% (backend registration TUs get dead-stripped).
if isempty(joined)
  fprintf('test_backend: OK (no backends registered)\n');
  return;
end
assert_ptiff(ischar(joined) && isvector(joined), 'backend_names must be a char array');
parts = strsplit(joined, ', ');
assert_ptiff(~any(cellfun(@isempty, parts)), 'no empty backend name');
assert_ptiff(any(ismember(parts, 'tiff')), 'tiff backend must be registered');

fprintf('test_backend: OK (backends: %s)\n', joined);
end
