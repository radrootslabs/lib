{ pkgs }:
let
  toolchain = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
  stableVersion = toolchain.toolchain.channel;
  stableTargets = toolchain.toolchain.targets or [];
  stableExtensions = [
    "clippy"
    "rust-analyzer"
    "rust-src"
    "rustfmt"
  ];
  coverageExtensions = stableExtensions ++ [
    "llvm-tools-preview"
  ];
in
{
  stable = pkgs.rust-bin.stable.${stableVersion}.default.override {
    extensions = stableExtensions;
    targets = stableTargets;
  };

  coverage = pkgs.rust-bin.selectLatestNightlyWith (
    nightly:
    nightly.default.override {
      extensions = coverageExtensions;
      targets = stableTargets;
    }
  );
}
