{ common, config, pkgs, toolchains }:
let
  stablePath = "export PATH=${toolchains.stable}/bin:$PATH";
  coveragePath = "export PATH=${toolchains.stable}/bin:${toolchains.coverage}/bin:$PATH";
  mkRepoApp =
    {
      name,
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
    };
in
{
  check = mkRepoApp {
    name = "check";
    runtimeInputs = common.runtimeInputs.stable;
    command = common.checkCommand;
  };

  contract = mkRepoApp {
    name = "contract";
    runtimeInputs = common.runtimeInputs.stable;
    command = common.contractCommand;
  };

  coverage-report = mkRepoApp {
    name = "coverage-report";
    runtimeInputs = common.runtimeInputs.coverage;
    command = common.coverageReportCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  export-ts = mkRepoApp {
    name = "export-ts";
    runtimeInputs = common.runtimeInputs.stable;
    command = ''
      cargo run -q -p xtask -- sdk export-ts "$@"
    '';
  };

  fmt = mkRepoApp {
    name = "fmt";
    runtimeInputs = common.runtimeInputs.stable ++ [
      config.treefmt.build.wrapper
    ];
    command = ''
      cargo fmt --all
      ${config.treefmt.build.wrapper}/bin/treefmt
    '';
  };

  publish-crates = mkRepoApp {
    name = "publish-crates";
    runtimeInputs = common.runtimeInputs.release;
    command = ''
      ./publish-crates.sh "$@"
    '';
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  publish-dry-run = mkRepoApp {
    name = "publish-dry-run";
    runtimeInputs = common.runtimeInputs.release;
    command = ''
      ./publish-crates.sh --dry-run "$@"
    '';
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  release-preflight = mkRepoApp {
    name = "release-preflight";
    runtimeInputs = common.runtimeInputs.coverage;
    command = common.releasePreflightCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  wasm-builds = mkRepoApp {
    name = "wasm-builds";
    runtimeInputs = common.runtimeInputs.wasm;
    command = common.wasmBuildsCommand;
  };
}
