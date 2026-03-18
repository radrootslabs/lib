{
  description = "Radroots Core Libraries";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.treefmt-nix.flakeModule ];
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      perSystem =
        {
          config,
          lib,
          system,
          ...
        }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };
          toolchains = import ./nix/toolchains.nix { inherit pkgs; };
          common = import ./nix/common.nix {
            crane = inputs.crane;
            inherit lib pkgs toolchains;
          };
        in
        {
          treefmt = import ./treefmt.nix;

          apps = import ./nix/apps.nix {
            inherit common config pkgs toolchains;
          };

          checks = lib.filterAttrs (_: value: value != null) (
            import ./nix/checks.nix {
              inherit common pkgs;
            }
          );

          devShells = import ./nix/devshells.nix {
            inherit common pkgs toolchains;
          };

          packages = {
            xtask = common.xtaskPackage;
          };
        };
    };
}
