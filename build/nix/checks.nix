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
        cargo check --workspace --all-targets
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
        cargo test ${common.coreContractCargoArgs}
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  mkReplicaSyncLane =
    {
      pname,
      command,
    }:
    common.craneLib.mkCargoDerivation (
      common.commonCraneArgs
      // {
        inherit (common) cargoArtifacts;
        inherit pname;
        doCheck = false;
        buildPhaseCargoCommand = command;
        installPhaseCommand = "mkdir -p $out";
      }
    );
  replicaSyncDefaultCheck = mkReplicaSyncLane {
    pname = "radroots-replica-sync-default-check";
    command = "cargo check -p radroots_replica_sync --all-targets";
  };
  replicaSyncDefaultTest = mkReplicaSyncLane {
    pname = "radroots-replica-sync-default-test";
    command = "cargo test -p radroots_replica_sync";
  };
  replicaSyncLegacyCheck = mkReplicaSyncLane {
    pname = "radroots-replica-sync-legacy-ingest-check";
    command = "cargo check -p radroots_replica_sync --all-targets --features legacy-ingest";
  };
  replicaSyncLegacyTest = mkReplicaSyncLane {
    pname = "radroots-replica-sync-legacy-ingest-test";
    command = "cargo test -p radroots_replica_sync --features legacy-ingest";
  };
in
{
  cargo-fmt = cargoFmt;
  cargo-check = cargoCheck;
  cargo-test = cargoTest;
  replica-sync-default-check = replicaSyncDefaultCheck;
  replica-sync-default-test = replicaSyncDefaultTest;
  replica-sync-legacy-ingest-check = replicaSyncLegacyCheck;
  replica-sync-legacy-ingest-test = replicaSyncLegacyTest;

  architecture = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-architecture";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo run --locked -q -p xtask -- architecture-ci
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );

  guards = common.mkRepoCheck {
    name = "repo-guards";
    runtimeInputs = [
      common.xtaskPackage
      pkgs.coreutils
      pkgs.gitMinimal
      pkgs.gnugrep
      pkgs.ripgrep
    ];
    initGit = true;
    command = ''
      xtask hygiene forbidden-identifiers
    '';
  };
}
