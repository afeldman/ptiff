function p = test_ptiff_fixture_path()
%TEST_PTIFF_FIXTURE_PATH  Absolute path to the C++-Oracle interop fixture.
%
%   Uses the frozen `scripts/samples/ptiff_interop_fixture.tif` the Phase-7
%   cross-language tests validate against (ptiff.camera.model=pinhole,
%   ptiff.spice.frame=IAU_MOON, intrinsics-only camera).
p = fullfile(fileparts(mfilename('fullpath')), '..', '..', '..', ...
    'scripts', 'samples', 'ptiff_interop_fixture.tif');
end
