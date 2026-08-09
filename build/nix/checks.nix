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
        cargo check --locked --all-targets ${common.publicNativeCargoArgs}
        cargo check --locked --all-targets ${common.previewCargoArgs}
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
        cargo test --locked ${common.coreContractCargoArgs}
        cargo test --locked ${common.previewCargoArgs}
        cargo clippy --locked --all-targets ${common.publicNativeCargoArgs} -- -D warnings
        cargo clippy --locked --all-targets ${common.previewCargoArgs} -- -D warnings
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
        cargo check -p radroots_core --all-targets --no-default-features --locked
        cargo check -p radroots_core --all-targets --no-default-features --features std --locked
        cargo check -p radroots_core --all-targets --no-default-features --features serde --locked
        cargo check -p radroots_core --all-targets --locked
        cargo check -p radroots_core --all-targets --all-features --locked
        cargo clippy -p radroots_core --all-targets --all-features --locked -- -D warnings
        cargo test -p radroots_core --all-targets --all-features --locked
        cargo check -p radroots_core --no-default-features --target wasm32-unknown-unknown --locked
        cargo check -p radroots_core --no-default-features --features serde --target wasm32-unknown-unknown --locked
        RUSTDOCFLAGS="-D warnings" cargo doc -p radroots_core --all-features --no-deps --locked
        cargo test -p radroots_core --all-features --doc --locked
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  identityConformance = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-identity-conformance";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check -p radroots_identity --all-targets --no-default-features --locked
        cargo check -p radroots_identity --all-targets --no-default-features --features std --locked
        cargo check -p radroots_identity --all-targets --no-default-features --features serde --locked
        cargo check -p radroots_identity --all-targets --locked
        cargo check -p radroots_identity --all-targets --all-features --locked
        cargo clippy -p radroots_identity --all-targets --no-default-features --locked -- -D warnings
        cargo clippy -p radroots_identity --all-targets --no-default-features --features serde --locked -- -D warnings
        cargo clippy -p radroots_identity --all-targets --all-features --locked -- -D warnings
        cargo test -p radroots_identity --all-targets --no-default-features --locked
        cargo test -p radroots_identity --all-targets --no-default-features --features std --locked
        cargo test -p radroots_identity --all-targets --no-default-features --features serde --locked
        cargo test -p radroots_identity --all-targets --all-features --locked
        cargo check -p radroots_identity --no-default-features --target wasm32-unknown-unknown --locked
        cargo check -p radroots_identity --no-default-features --features serde --target wasm32-unknown-unknown --locked
        RUSTDOCFLAGS="-D warnings" cargo doc -p radroots_identity --all-features --no-deps --locked
        cargo test -p radroots_identity --all-features --doc --locked
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  blossomConformance = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-blossom-conformance";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check -p radroots_blossom --all-targets --no-default-features --locked
        cargo check -p radroots_blossom --all-targets --no-default-features --features std --locked
        cargo check -p radroots_blossom --all-targets --no-default-features --features serde --locked
        cargo check -p radroots_blossom --all-targets --locked
        cargo check -p radroots_blossom --all-targets --all-features --locked
        cargo clippy -p radroots_blossom --all-targets --no-default-features --locked -- -D warnings
        cargo clippy -p radroots_blossom --all-targets --no-default-features --features serde --locked -- -D warnings
        cargo clippy -p radroots_blossom --all-targets --all-features --locked -- -D warnings
        cargo test -p radroots_blossom --all-targets --no-default-features --locked
        cargo test -p radroots_blossom --all-targets --no-default-features --features std --locked
        cargo test -p radroots_blossom --all-targets --no-default-features --features serde --locked
        cargo test -p radroots_blossom --all-targets --all-features --locked
        cargo check -p radroots_blossom --no-default-features --target wasm32-unknown-unknown --locked
        cargo check -p radroots_blossom --no-default-features --features serde --target wasm32-unknown-unknown --locked
        RUSTDOCFLAGS="-D warnings" cargo doc -p radroots_blossom --all-features --no-deps --locked
        cargo test -p radroots_blossom --all-features --doc --locked
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  eventConformance = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-event-conformance";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check -p radroots_event --all-targets --no-default-features --locked
        cargo check -p radroots_event --all-targets --no-default-features --features std --locked
        cargo check -p radroots_event --all-targets --no-default-features --features serde --locked
        cargo check -p radroots_event --all-targets --no-default-features --features knowledge --locked
        cargo check -p radroots_event --all-targets --no-default-features --features std,knowledge --locked
        cargo check -p radroots_event --all-targets --no-default-features --features serde,knowledge --locked
        cargo check -p radroots_event --all-targets --locked
        cargo check -p radroots_event --all-targets --all-features --locked
        cargo clippy -p radroots_event --all-targets --no-default-features --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --no-default-features --features std --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --no-default-features --features serde --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --no-default-features --features knowledge --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --no-default-features --features std,knowledge --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --no-default-features --features serde,knowledge --locked -- -D warnings
        cargo clippy -p radroots_event --all-targets --all-features --locked -- -D warnings
        cargo test -p radroots_event --all-targets --no-default-features --locked
        cargo test -p radroots_event --all-targets --no-default-features --features std --locked
        cargo test -p radroots_event --all-targets --no-default-features --features serde --locked
        cargo test -p radroots_event --all-targets --no-default-features --features knowledge --locked
        cargo test -p radroots_event --all-targets --no-default-features --features std,knowledge --locked
        cargo test -p radroots_event --all-targets --no-default-features --features serde,knowledge --locked
        cargo test -p radroots_event --all-targets --locked
        cargo test -p radroots_event --all-targets --all-features --locked
        cargo check -p radroots_event --no-default-features --target wasm32-unknown-unknown --locked
        cargo check -p radroots_event --no-default-features --features serde --target wasm32-unknown-unknown --locked
        cargo check -p radroots_event --no-default-features --features knowledge --target wasm32-unknown-unknown --locked
        cargo check -p radroots_event --no-default-features --features serde,knowledge --target wasm32-unknown-unknown --locked
        cargo check -p radroots_event --all-features --target wasm32-unknown-unknown --locked
        RUSTDOCFLAGS="-D warnings" cargo doc -p radroots_event --all-features --no-deps --locked
        cargo test -p radroots_event --all-features --doc --locked
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
  identity-conformance = identityConformance;
  blossom-conformance = blossomConformance;
  event-conformance = eventConformance;
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
        cargo run --locked -q -p xtask -- architecture-source-export-ci
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
      xtask hygiene prototype-contracts
    '';
  };
}
