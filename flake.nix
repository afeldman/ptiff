{
  description = "PTIFF (Planetary TIFF) image format library + CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        ptiff = (import ./default.nix { inherit pkgs; version = "1.0.0"; });
      in {
        packages = {
          default = ptiff;
          inherit ptiff;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ ptiff ];
          packages = with pkgs; [ clang-tools cmake-format ];
        };
      });
}
