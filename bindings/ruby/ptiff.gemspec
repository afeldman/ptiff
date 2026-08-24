# frozen_string_literal: true

require_relative 'lib/ptiff/version'

Gem::Specification.new do |spec|
  spec.name = 'ptiff'
  spec.version = PTiff::VERSION
  spec.authors = ['Anton Feldmann']
  spec.summary = 'Ruby bindings for libptiff (planetary imaging): raw SWIG output over libptiff_c'
  spec.description = <<~DESC
    Ruby bindings for libptiff, a C++23 planetary-imaging library: raw SWIG
    output (bindings/swig/ptiff.i) over the language-agnostic C ABI
    `libptiff_c`. `lib/ptiff.{bundle,so}` is a compiled native extension linked
    against `libptiff_c` -- regenerate it with `make -C ../swig ruby` before
    requiring this gem; it is not pure Ruby and ships no prebuilt binary.
  DESC
  spec.homepage = 'https://github.com/afeldman/ptiff'
  spec.license = 'Apache-2.0'
  spec.required_ruby_version = '>= 3.0'

  spec.files = Dir[
    'lib/**/*.rb',
    'lib/**/*.bundle',
    'lib/**/*.so',
    'README.md',
    'LICENSE*',
    'ptiff.gemspec'
  ]
  spec.require_paths = ['lib']

  # Only the Ruby standard library is needed at runtime (ptiff.bundle links
  # directly against libptiff_c); minitest/rake are dev-only.
  spec.add_development_dependency 'minitest', '~> 5.0'
  spec.add_development_dependency 'rake', '~> 13.0'
end
