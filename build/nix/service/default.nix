{
  crane,
  lib,
  pkgs,
}:
{
  supportedSystems = import ./systems.nix;
  mkToolchain = import ./toolchain.nix { inherit pkgs; };
  mkNativeInputs = import ./native-inputs.nix { inherit lib; };
  mkServicePackage = import ./package.nix { inherit crane lib pkgs; };
  mkServiceChecks = import ./checks.nix { inherit crane lib pkgs; };
  mkServiceApps = import ./apps.nix { inherit lib pkgs; };
  mkServiceDevShell = import ./devshell.nix { inherit lib pkgs; };
  mkServiceOciImage = import ./oci.nix { inherit lib pkgs; };
  mkServiceNixosModule = import ./nixos-module.nix { inherit lib; };
  mkServiceOutputs = import ./compose.nix { inherit lib; };
}
