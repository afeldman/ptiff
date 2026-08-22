# frozen_string_literal: true

# Version surface over the raw SWIG-generated Ruby binding.
#
# Ports bindings/ruby/test/test_ptiff.rb's test_runtime_version and
# test_runtime_matches_compile_time onto Ptiff::ptiff_runtime_version /
# Ptiff::ptiff_compile_time_version, plus the out-parameter forms
# (Ptiff::ptiff_runtime_version_out / ptiff_compile_time_version_out).
# The *_out forms needed the OUTPUT typemap added to typemaps.i (SWIGRUBY
# block, "int *major"/"int *minor"/"int *patch") to surface as extra return
# values via SWIG_Ruby_AppendOutput -- without it there was no way to pass a
# bare int* by reference from pure Ruby.
require 'minitest/autorun'
require 'ptiff'

class TestVersion < Minitest::Test
  def test_runtime_version_fields
    # Keep in sync with the project version (bindings/rust/Cargo.toml, CHANGELOG.md).
    # Do NOT hard-code a specific minor here and forget to bump it on release —
    # the cross-check below (runtime == compile_time) is version-agnostic.
    v = Ptiff::ptiff_runtime_version
    assert_equal 0, v.major
    assert_equal 3, v.minor
    assert_equal 0, v.patch
  end

  def test_runtime_matches_compile_time
    r = Ptiff::ptiff_runtime_version
    c = Ptiff::ptiff_compile_time_version
    assert_equal [r.major, r.minor, r.patch], [c.major, c.minor, c.patch]
  end

  def test_version_out_params
    r_maj, r_min, r_pat = Ptiff::ptiff_runtime_version_out
    c_maj, c_min, c_pat = Ptiff::ptiff_compile_time_version_out
    assert_equal [r_maj, r_min, r_pat], [c_maj, c_min, c_pat]

    v = Ptiff::ptiff_runtime_version
    assert_equal [r_maj, r_min, r_pat], [v.major, v.minor, v.patch]
  end
end
