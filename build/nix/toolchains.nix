{ pkgs }:
{
  stable = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;

  coverage = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain-coverage.toml;

  fuzz = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain-fuzz.toml;

  ios = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain-ios.toml;
}
