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
  coreConformance = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-core-conformance";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check -p radroots-core --all-targets --no-default-features --locked
        cargo check -p radroots-core --all-targets --no-default-features --features std --locked
        cargo check -p radroots-core --all-targets --no-default-features --features serde --locked
        cargo check -p radroots-core --all-targets --locked
        cargo check -p radroots-core --all-targets --all-features --locked
        cargo clippy -p radroots-core --all-targets --all-features --locked -- -D warnings
        cargo test -p radroots-core --all-targets --all-features --locked
        cargo check -p radroots-core --no-default-features --target wasm32-unknown-unknown --locked
        cargo check -p radroots-core --no-default-features --features serde --target wasm32-unknown-unknown --locked
        RUSTDOCFLAGS="-D warnings" cargo doc -p radroots-core --all-features --no-deps --locked
        cargo test -p radroots-core --all-features --doc --locked
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
  core-conformance = coreConformance;
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
