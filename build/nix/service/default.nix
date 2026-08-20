{ lib, pkgs }:
{
  supportedSystems = import ./systems.nix;
  mkToolchain = import ./toolchain.nix { inherit pkgs; };
  mkNativeInputs = import ./native-inputs.nix { inherit lib; };
  mkServiceOutputs = import ./compose.nix { inherit lib; };
}
