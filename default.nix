# -*- mode: nix -*-
# Nix package definition for PTIFF (Planetary TIFF).
#
# PTIFF 1.0 is implemented entirely in Rust: `cargo` is the real build. The
# package installs the Rust `ptiff` CLI (crates/ptiff-cli) and the C ABI
# `libptiff_c` (crates/ptiff-c) plus its generated header, so FFI consumers
# (Go/Python/Ruby/Octave SWIG bindings, or any C program) can link against it.
# There is no C++ library any more, so none of the old C++ build inputs
# (clang, fmt, spdlog, pugixml, OpenEXR, libcurl, ...) are needed.

{ pkgs ? import <nixpkgs> { }
, version ? "1.1.0"
}:

let
  # A small pkg-config file for the C ABI, rendered as a Nix string.
  pcContent = pkgs.lib.concatStringsSep "\n" [
    ("prefix=${"$"}{pcfiledir}/../..")
    ("libdir=${"$"}{prefix}/lib")
    ("includedir=${"$"}{prefix}/include")
    ""
    "Name: ptiff_c"
    "Description: PTIFF 1.0 C ABI over the Rust core"
    ("Version: ${version}")
    ("Libs: -L${"$"}{libdir} -lptiff_c")
    ("Cflags: -I${"$"}{includedir}")
    ""
  ];
in
pkgs.stdenv.mkDerivation {
  pname = "ptiff";
  inherit version;

  src = pkgs.lib.cleanSource ./.;

  nativeBuildInputs = with pkgs; [
    rustc
    cargo
  ];

  # Clean up generated/scratch artifacts that would otherwise be copied into
  # the Nix store.
  postPatch = ''
    rm -rf build build-rel build-debug target
  '';

  buildPhase = ''
    runHook preBuild
    # Build the C ABI (libptiff_c) and CLI (ptiff) in release mode. CARGO_TARGET_DIR
    # keeps Cargo's output inside the Nix build sandbox rather than the source tree.
    export CARGO_TARGET_DIR="$TMPDIR/cargo-target"
    cargo build --release -p ptiff-c
    cargo build --release -p ptiff-cli
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p "$out/bin" "$out/lib" "$out/include" "$out/lib/pkgconfig"

    # CLI
    install -m 0755 "$CARGO_TARGET_DIR/release/ptiff" "$out/bin/ptiff"

    # C ABI: shared + static library (shared suffix varies by platform).
    install -m 0755 "$CARGO_TARGET_DIR"/release/libptiff_c.so* "$out/lib/" 2>/dev/null || true
    install -m 0755 "$CARGO_TARGET_DIR"/release/libptiff_c.dylib "$out/lib/" 2>/dev/null || true
    install -m 0644 "$CARGO_TARGET_DIR/release/libptiff_c.a" "$out/lib/"

    # Generated header (cbindgen writes a stable copy to CARGO_TARGET_DIR).
    install -m 0644 "$CARGO_TARGET_DIR/ptiff_c.h" "$out/include/ptiff_c.h"

    # pkg-config file for C/FFI consumers.
    printf '%s' "${pcContent}" > "$out/lib/pkgconfig/ptiff_c.pc"

    runHook postInstall
  '';

  meta = with pkgs.lib; {
    description = "Planetary TIFF (PTIFF) image format: Rust core, C ABI and CLI";
    homepage = "https://afeldman.github.io/ptiff/";
    license = licenses.asl20;
    platforms = platforms.linux ++ platforms.darwin;
  };
}
