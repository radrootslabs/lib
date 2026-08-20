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

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.treefmt-nix.flakeModule ];
      systems = import ./build/nix/service/systems.nix;

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
          service = import ./build/nix/service {
            crane = inputs.crane;
            inherit lib pkgs;
          };
          toolchains = {
            stable = service.mkToolchain {
              rustToolchainFile = ./rust-toolchain.toml;
            };
            coverage = service.mkToolchain {
              rustToolchainFile = ./rust-toolchain-coverage.toml;
            };
          };
          common = import ./build/nix/common.nix {
            crane = inputs.crane;
            inherit lib pkgs toolchains;
          };
          serviceFixture = import ./build/nix/service/fixture.nix {
            inherit lib pkgs service;
            toolchain = toolchains.stable;
          };
        in
        {
          treefmt = import ./treefmt.nix;

          apps =
            (import ./build/nix/apps.nix {
              inherit
                common
                config
                lib
                pkgs
                toolchains
                ;
            })
            // serviceFixture.outputs.apps;

          checks = lib.filterAttrs (_: value: value != null) (
            (import ./build/nix/checks.nix {
              inherit common pkgs;
            })
            // serviceFixture.outputs.checks
          );

          devShells =
            (import ./build/nix/devshells.nix {
              inherit common pkgs toolchains;
            })
            // {
              service-fixture = serviceFixture.outputs.devShells.default;
            };

          packages = serviceFixture.outputs.packages // {
            xtask = common.xtaskPackage;
          };
        };
    };
}
