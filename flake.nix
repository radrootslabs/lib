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
    inputs@{
      self,
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.treefmt-nix.flakeModule ];
      systems = import ./build/nix/service/systems.nix;

      flake.lib = {
        supportedSystems = import ./build/nix/service/systems.nix;
        mkServiceHelpers =
          system:
          assert inputs.nixpkgs.lib.assertMsg (builtins.elem system (
            import ./build/nix/service/systems.nix
          )) "service helpers support only the governed Nix systems";
          let
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [ inputs.rust-overlay.overlays.default ];
            };
          in
          import ./build/nix/service {
            crane = inputs.crane;
            lib = inputs.nixpkgs.lib;
            inherit pkgs;
          };
      };

      flake.overlays.default = final: _previous: {
        radroots-lib =
          assert inputs.nixpkgs.lib.assertMsg
            (builtins.elem final.stdenv.hostPlatform.system self.lib.supportedSystems)
            "the Radroots Lib overlay supports only the governed Nix systems";
          self.packages.${final.stdenv.hostPlatform.system}.default;
      };

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
          library = import ./build/nix/library.nix {
            inherit lib pkgs;
            inherit (common) version;
          };
          serviceFixture = import ./build/nix/service/fixture.nix {
            inherit lib pkgs service;
            nixosSystem = inputs.nixpkgs.lib.nixosSystem;
            toolchain = toolchains.stable;
          };
          fixtureChecks = lib.mapAttrs' (
            name: value: lib.nameValuePair "service-fixture-${name}" value
          ) serviceFixture.outputs.checks;
        in
        {
          treefmt = import ./treefmt.nix;

          apps = {
            default = library.app;
          }
          // (import ./build/nix/apps.nix {
            inherit
              common
              config
              lib
              pkgs
              toolchains
              ;
          });

          checks = lib.filterAttrs (_: value: value != null) (
            (import ./build/nix/checks.nix {
              inherit common pkgs;
            })
            // fixtureChecks
            // {
              release-bundle = library.check;
            }
          );

          devShells = import ./build/nix/devshells.nix {
            inherit common pkgs toolchains;
          };

          packages = {
            default = library.package;
            xtask = common.xtaskPackage;
          };
        };
    };
}
