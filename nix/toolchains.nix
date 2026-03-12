{ pkgs }:
let
  toolchain = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
  stableVersion = toolchain.toolchain.channel;
  stableTargets = toolchain.toolchain.targets or [];
  extensions = [
    "clippy"
    "rust-analyzer"
    "rust-src"
    "rustfmt"
  ];
in
{
  stable = pkgs.rust-bin.stable.${stableVersion}.default.override {
    inherit extensions;
    targets = stableTargets;
  };

  coverage = pkgs.rust-bin.selectLatestNightlyWith (
    nightly:
    nightly.default.override {
      inherit extensions;
      targets = stableTargets;
    }
  );
}
