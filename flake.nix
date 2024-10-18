{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.rustup
            pkgs.cargo-insta
            pkgs.gcc14
            pkgs.glibc_multi.dev
          ];

          # Prevent nix from messing with the test suite.
          shellHook = ''
            unset NIX_LDFLAGS NIX_HARDENING_ENABLE
          '';
        };
      }
    );
}
