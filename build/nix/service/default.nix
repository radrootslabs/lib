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
  mkServiceOutputs = import ./compose.nix { inherit lib; };
}
