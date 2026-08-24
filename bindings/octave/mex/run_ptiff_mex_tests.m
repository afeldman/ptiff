function run_ptiff_mex_tests()
%RUN_PTIFF_MEX_TESTS  Run the ptiff-octave MEX test suite.
%
%   Discovers every test_ptiff_mex_*.m function in this file's directory and
%   calls it in sequence (each is a self-contained function with no shared
%   workspace). Each test ends by printing `test_ptiff_mex_<x>: OK`; a
%   failure aborts with that function's error.
%
%   This is the entry point `make test` uses in bindings/octave/mex/:
%
%       octave --no-gui --eval "addpath('.') run_ptiff_mex_tests();"

this = fileparts(mfilename('fullpath'));
files = dir(fullfile(this, 'test_ptiff_mex_*.m'));
names = cellfun(@(f) f(1:end-2), {files.name}, 'UniformOutput', false);
names = sort(names);
names = names(~strcmp(names, 'run_ptiff_mex_tests'));

n = numel(names);
fprintf('== ptiff-octave MEX suite: %d tests ==\n', n);
for i = 1:n
  fprintf('-- running %s (%d/%d)\n', names{i}, i, n);
  feval(names{i});
end
fprintf('== all %d ptiff-octave MEX tests passed ==\n', n);
end
