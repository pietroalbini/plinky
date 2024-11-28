{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        pkgs32 = nixpkgs.legacyPackages."i686-linux";
        pkgs64 = nixpkgs.legacyPackages."x86_64-linux";
      in
      {
        devShells.default = pkgs.mkShellNoCC {
          packages = [
            pkgs.cargo-insta
            pkgs.gcc14
            pkgs.nasm
            pkgs.rustup

            # Provide headers for both 32bit and 64bit:
            pkgs32.glibc.dev
            pkgs64.glibc.dev
          ];

          PLINKY_TEST_DYNAMIC_LINKER_32 = "${pkgs32.glibc}/lib/ld-linux.so.2";
          PLINKY_TEST_DYNAMIC_LINKER_64 = "${pkgs64.glibc}/lib/ld-linux-x86-64.so.2";

          shellHook = ''
            # Prevent nix from messing with the test suite.
            unset NIX_LDFLAGS NIX_HARDENING_ENABLE

            # Put the test files inside of the target dir to keep them inside of the dev shell.
            export TMPDIR="$(git rev-parse --show-toplevel)/target/tmp"
          '';
        };
      }
    );
}
