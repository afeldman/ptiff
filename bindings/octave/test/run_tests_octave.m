function run_tests_octave()
%RUN_TESTS_OCTAVE  Run the full ptiff Octave test suite.
%
%   Finds every test_*.m function in this file's directory and calls it in
%   sequence. Each test is a function (no shared workspace), so it is also
%   safe to run them in any order and independently. Each test ends by
%   printing `test_<x>: OK`; if one throws, the run aborts with that error.
%
%   Before calling the tests it loads the SWIG module once via ptiff()
%   (in this base workspace) so the module's global constants exist for any
%   test that reads them through `global` declarations.
%
%   This is the single entry point `make test` uses:
%
%       octave --quiet --no-gui -p <octave mod dir> -p <octave test dir> \
%         --eval "<this function name>()"     (or run this file)

this = fileparts(mfilename('fullpath'));
files = dir(fullfile(this, 'test_*.m'));
names = cellfun(@(f) f(1:end-2), {files.name}, 'UniformOutput', false);
names = sort(names);
% drop this runner itself (defensive; it does not match test_ but keep it cheap)
names = names(~strcmp(names, 'run_tests_octave'));

% Load the module once in this base workspace (guarded: subsequent calls are
% a no-op because by then `ptiff` is the SWIG namespace object, not the
% loadable-function entry point).
if exist('ptiff', 'var') == 0
  ptiff();
end

n = numel(names);
fprintf('== ptiff Octave suite: %d tests ==\n', n);
for i = 1:n
  fprintf('-- running %s (%d/%d)\n', names{i}, i, n);
  feval(names{i});
end
fprintf('== all %d ptiff Octave tests passed ==\n', n);
end
