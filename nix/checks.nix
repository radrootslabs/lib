{ common, pkgs }:
let
  cargoFmt = common.craneLib.cargoFmt common.commonCraneArgs;
  cargoCheck = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-cargo-check";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check ${common.sdkContractCargoArgs}
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  cargoTest = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-cargo-test";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo test ${common.sdkContractCargoArgs}
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  actionlint = common.mkRepoCheck {
    name = "actionlint";
    runtimeInputs = [
      pkgs.actionlint
      pkgs.shellcheck
    ];
    initGit = true;
    command = ''
      actionlint
    '';
  };
in
{
  actionlint = actionlint;
  cargo-fmt = cargoFmt;
  cargo-check = cargoCheck;
  cargo-test = cargoTest;

  guards = common.mkRepoCheck {
    name = "repo-guards";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.gitMinimal
      pkgs.gnugrep
    ];
    initGit = true;
    command = ''
      ./scripts/ci/guard_committed_ts_artifacts.sh
      ./scripts/ci/guard_no_legacy_identifiers.sh
    '';
  };
}
