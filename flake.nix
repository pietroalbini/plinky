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
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShellNoCC {
          packages = [
            pkgs.rustup
            pkgs.cargo-insta
            pkgs.gcc14
            pkgs.glibc_multi.dev
          ];

          PLINKY_TEST_DYNAMIC_LINKER_32 = "${pkgs.glibc_multi}/lib/32/ld-linux.so.2";
          PLINKY_TEST_DYNAMIC_LINKER_64 = "${pkgs.glibc_multi}/lib/ld-linux-x86-64.so.2";

          # Prevent nix from messing with the test suite.
          shellHook = ''
            unset NIX_LDFLAGS NIX_HARDENING_ENABLE
          '';
        };
      }
    );
}
