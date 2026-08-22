# -*- mode: nix -*-
# Nix package definition for PTIFF (Planetary TIFF).
#
# Built with the user's preferred toolchain: the LLVM/clang stdenv (clangStdenv)
# and Ninja. The C/C++ library, the C ABI binding (libptiff_c) and the Rust CLI
# (ptiff-cli/pkg) are all installed into the standard Nix output layout. The
# C++ backend dependencies (fmt, spdlog, pugixml, nlohmann_json, OpenEXR, zstd,
# zlib, libdeflate, libjpeg-turbo, libcurl, cpp-httplib) come from nixpkgs, each
# shipping the CMake CONFIG package the build's find_package() calls require.
{ pkgs ? import <nixpkgs> { }
, version ? "0.4.2"
}:

pkgs.clangStdenv.mkDerivation {
  pname = "libptiff";
  inherit version;

  src = pkgs.lib.cleanSource ./.;

  # Clean-up a handful of generated/scratch artifacts that would otherwise be
  # copied from the working tree into the Nix store.
  postPatch = ''
    rm -rf build build-rel build-debug target
    rm -f ptiff-cli/target ptiff-cli/Cargo.lock
  '';

  nativeBuildInputs = with pkgs; [
    ninja
    cmake
    pkg-config
    rustc
    cargo
  ];

  buildInputs = with pkgs; [
    fmt
    spdlog
    catch2_3
    pugixml
    nlohmann_json
    openexr
    zstd
    zlib
    libdeflate
    libjpeg-turbo
    curl
    cpp-httplib
  ];

  dontFixup = false;

  cmakeFlags = [
    "-G Ninja"
    "-DCMAKE_BUILD_TYPE=Release"
    "-DBUILD_SHARED_LIBS=ON"
    "-DPTIFF_BUILD_C_BINDINGS=ON"
    "-DPTIFF_BUILD_CLI=ON"
    "-DPTIFF_BUILD_TESTS=OFF"
    "-DPTIFF_BUILD_DOCS=OFF"
    "-DPTIFF_BUILD_EXAMPLES=OFF"
    "-DPTIFF_BUILD_BENCHMARKS=OFF"
  ];

  # The Rust CLI links against an installed libptiff_c (pkg-config). Point it at
  # the Nix output *before* install completes is not possible, so we rely on the
  # default Nix configure/build/install cycle: cmake configures (the CLI target
  # is NOT ALL, so it is not built during `cmake --build`), cpack is not run, and
  # the CLI is built/installed explicitly afterwards against $out.
  buildPhase = ''
    cmake --build build
  '';

  installPhase = ''
    cmake --install build --prefix "$out"
    # Build the Rust CLI against the just-installed prefix and place it in bin/.
    PKG_CONFIG_PATH="$out/lib/pkgconfig" \
      cargo build --release --manifest-path "$PWD/ptiff-cli/Cargo.toml"
    mkdir -p "$out/bin"
    install -m 0755 "$PWD/ptiff-cli/target/release/ptiff" "$out/bin/ptiff"
  '';

  meta = with pkgs.lib; {
    description = "Planetary TIFF (PTIFF) image format C/C++ library and CLI";
    homepage = "https://afeldman.github.io/ptiff/";
    license = licenses.asl20;
    platforms = platforms.linux ++ platforms.darwin;
  };
}
