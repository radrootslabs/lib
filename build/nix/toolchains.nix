{ pkgs }:
{
  stable = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;

  coverage = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain-coverage.toml;
}
