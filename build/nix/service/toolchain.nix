{ pkgs }:
{ rustToolchainFile }:
assert builtins.pathExists rustToolchainFile;
pkgs.rust-bin.fromRustupToolchainFile rustToolchainFile
