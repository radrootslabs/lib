{ common, config, pkgs, toolchains }:
let
  stablePath = "export PATH=${toolchains.stable}/bin:$PATH";
  coveragePath = "export PATH=${toolchains.stable}/bin:${toolchains.coverage}/bin:$PATH";
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
    description = "Run the sdk contract lane and export validation";
    runtimeInputs = common.runtimeInputs.stable;
    command = common.contractCommand;
  };

  coverage-report = mkRepoApp {
    name = "coverage-report";
    description = "Generate sdk coverage reports and blocking gate artifacts";
    runtimeInputs = common.runtimeInputs.coverage;
    command = common.coverageReportCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  export-ts = mkRepoApp {
    name = "export-ts";
    description = "Export generated typescript sdk artifacts";
    runtimeInputs = common.runtimeInputs.stable;
    command = ''
      cargo run -q -p xtask -- sdk export-ts "$@"
    '';
  };

  guards = mkRepoApp {
    name = "guards";
    description = "Run repository guard scripts";
    runtimeInputs = common.runtimeInputs.stable;
    command = ''
      ./scripts/ci/guard_committed_ts_artifacts.sh
      ./scripts/ci/guard_no_legacy_identifiers.sh
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

  publish-crates = mkRepoApp {
    name = "publish-crates";
    description = "Publish crates through the workspace release script";
    runtimeInputs = common.runtimeInputs.release;
    command = ''
      ./publish-crates.sh "$@"
    '';
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  publish-dry-run = mkRepoApp {
    name = "publish-dry-run";
    description = "Run a dry-run crates publish through the workspace release script";
    runtimeInputs = common.runtimeInputs.release;
    command = ''
      ./publish-crates.sh --dry-run "$@"
    '';
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  release-preflight = mkRepoApp {
    name = "release-preflight";
    description = "Run release coverage refresh and preflight validation";
    runtimeInputs = common.runtimeInputs.coverage;
    command = common.releasePreflightCommand;
    env = common.exportCoverageEnv;
    pathPrefix = coveragePath;
  };

  validate-sdk-typescript = mkRepoApp {
    name = "validate-sdk-typescript";
    description = "Validate the synced sdk-typescript checkout with bun";
    runtimeInputs = common.runtimeInputs.sync;
    command = common.validateSdkTypescriptCommand;
  };

  wasm-builds = mkRepoApp {
    name = "wasm-builds";
    description = "Build the wasm packages defined by the workspace makefile";
    runtimeInputs = common.runtimeInputs.wasm;
    command = common.wasmBuildsCommand;
  };
}
