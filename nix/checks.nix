{ common, pkgs }:
let
  cargoFmt = common.craneLib.cargoFmt common.commonCraneArgs;
  cargoCheck = common.craneLib.cargoCheck (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      cargoExtraArgs = common.sdkContractCargoArgs;
    }
  );
  cargoTest = common.craneLib.cargoTest (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      cargoExtraArgs = common.sdkContractCargoArgs;
    }
  );
in
{
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
