{
  common,
  config,
  lib,
  pkgs,
  toolchains,
}:
let
  stablePath = "export PATH=${toolchains.stable}/bin:$PATH";
  coveragePath = "export PATH=${toolchains.stable}/bin:${toolchains.coverage}/bin:$PATH";
  coverageShellExec =
    name: command:
    let
      scriptName = "${name}-coverage-shell";
      script = pkgs.writeShellApplication {
        name = scriptName;
        runtimeInputs = common.runtimeInputs.coverage;
        text = command;
      };
    in
    ''
      exec nix develop .#coverage --accept-flake-config -c ${script}/bin/${scriptName} "$@"
    '';
  mkRepoApp =
    {
      name,
      description ? "Run ${name} in the radroots workspace",
      runtimeInputs,
      command,
      env ? common.exportSharedEnv,
      pathPrefix ? stablePath,
    }:
    let
      script = pkgs.writeShellApplication {
        inherit name runtimeInputs;
        text = ''
          set -euo pipefail

          repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
          cd "$repo_root"

          ${common.ensureRepoRoot}
          ${env}
          ${pathPrefix}

          ${command}
        '';
      };
    in
    {
      type = "app";
      program = "${script}/bin/${name}";
      meta.description = description;
    };
in
{
  check = mkRepoApp {
    name = "check";
    description = "Run cargo check across the radroots workspace";
    runtimeInputs = common.runtimeInputs.stable;
    command = common.checkCommand;
  };

  contract = mkRepoApp {
    name = "contract";
    description = "Run the core-library contract lane";
    runtimeInputs = common.runtimeInputs.stable;
    command = common.contractCommand;
  };

  coverage-report = mkRepoApp {
    name = "coverage-report";
    description = "Generate coverage reports and blocking gate artifacts";
    runtimeInputs = common.runtimeInputs.coverage;
    command = common.coverageReportCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  guards = mkRepoApp {
    name = "guards";
    description = "Run repository hygiene guards";
    runtimeInputs = common.runtimeInputs.stable;
    command = ''
      cargo run -q -p xtask -- hygiene forbidden-identifiers
    '';
  };

  fmt = mkRepoApp {
    name = "fmt";
    description = "Format rust, nix, shell, and toml files";
    runtimeInputs = common.runtimeInputs.stable ++ [
      config.treefmt.build.wrapper
    ];
    command = ''
      cargo fmt --all
      ${config.treefmt.build.wrapper}/bin/treefmt
    '';
  };

  release-preflight = mkRepoApp {
    name = "release-preflight";
    description = "Run release coverage refresh and preflight validation";
    runtimeInputs = [
      pkgs.nix
    ];
    command = coverageShellExec "release-preflight" common.releasePreflightCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

}
