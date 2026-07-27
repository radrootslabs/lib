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
      nativeBuildInputs = common.commonCraneArgs.nativeBuildInputs ++ [ pkgs.gitMinimal ];
      buildPhaseCargoCommand = ''
        cargo test ${common.coreContractCargoArgs}
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  blossomNoDefaultCheck = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-blossom-no-default-check";
      doCheck = false;
      buildPhaseCargoCommand = ''
        cargo check -p radroots_blossom --lib --no-default-features
        cargo check -p radroots_blossom --lib --no-default-features --features serde
        cargo check -p radroots_blossom --lib --no-default-features --features raster-decode
      '';
      installPhaseCommand = "mkdir -p $out";
    }
  );
  blossomRasterDecodeTest = common.craneLib.mkCargoDerivation (
    common.commonCraneArgs
    // {
      inherit (common) cargoArtifacts;
      pname = "radroots-blossom-raster-decode-test";
      doCheck = false;
      nativeBuildInputs = common.commonCraneArgs.nativeBuildInputs ++ [
        pkgs.imagemagick
        pkgs.jq
        pkgs.time
      ];
      buildPhaseCargoCommand = ''
        cargo test -p radroots_blossom --no-default-features --features serde \
          --test publication_readiness_persistence
        cargo test -p radroots_blossom --no-default-features --features raster-decode,serde
        ${common.decoderSecurityStableCommand}
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
  blossom-no-default-check = blossomNoDefaultCheck;
  blossom-raster-decode-test = blossomRasterDecodeTest;
  replica-sync-default-check = replicaSyncDefaultCheck;
  replica-sync-default-test = replicaSyncDefaultTest;
  replica-sync-legacy-ingest-check = replicaSyncLegacyCheck;
  replica-sync-legacy-ingest-test = replicaSyncLegacyTest;

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
