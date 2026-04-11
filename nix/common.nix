{
  crane,
  lib,
  pkgs,
  toolchains,
}:
let
  root = ../.;
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.workspace.package.version;
  darwinBuildInputs = lib.optionals pkgs.stdenv.isDarwin [
    pkgs.libiconv
  ];
  repoSource = lib.sources.cleanSource root;
  cargoSource = lib.fileset.toSource {
    root = root;
    fileset = lib.fileset.intersection (lib.fileset.fromSource repoSource) (
      lib.fileset.unions [
        ../Cargo.toml
        ../Cargo.lock
        ../Makefile
        ../README
        ../rust-toolchain.toml
        ../conformance
        ../contract
        ../crates
        ../scripts
      ]
    );
  };
  baseEnv = {
    CARGO_TERM_COLOR = "always";
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
  }
  // lib.optionalAttrs pkgs.stdenv.isDarwin {
    CC = "clang";
    CXX = "clang++";
    SDKROOT = pkgs.apple-sdk_14.sdkroot;
    MACOSX_DEPLOYMENT_TARGET = pkgs.stdenv.hostPlatform.darwinMinVersion;
  };
  sharedEnv =
    baseEnv
    // {
      PKG_CONFIG_PATH = lib.makeSearchPathOutput "dev" "lib/pkgconfig" stableRuntimeInputs;
    }
    // lib.optionalAttrs pkgs.stdenv.isDarwin {
      LIBRARY_PATH = lib.makeLibraryPath darwinBuildInputs;
    };
  coverageEnv = sharedEnv // {
    RADROOTS_COVERAGE_CARGO = "${toolchains.coverage}/bin/cargo";
  };
  cargoLlvmCov =
    (pkgs.callPackage "${pkgs.path}/pkgs/by-name/ca/cargo-llvm-cov/package.nix" { }).overrideAttrs
      (old: {
        doCheck = false;
        meta = old.meta // {
          broken = false;
        };
      });
  exportEnv =
    env:
    lib.concatStringsSep "\n" (
      lib.mapAttrsToList (name: value: "export ${name}=${lib.escapeShellArg value}") env
    );
  stableRuntimeInputs =
    with pkgs;
    [
      toolchains.stable
      clang
      coreutils
      curl
      findutils
      gawk
      gitMinimal
      gnugrep
      gnumake
      gnused
      jq
      libsodium
      llvmPackages.llvm
      llvmPackages.libclang
      pkg-config
      python3
    ]
    ++ darwinBuildInputs;
  syncRuntimeInputs = stableRuntimeInputs ++ [
    pkgs.bun
  ];
  coverageRuntimeInputs = stableRuntimeInputs ++ [
    toolchains.coverage
    cargoLlvmCov
  ];
  wasmRuntimeInputs = stableRuntimeInputs ++ [
    pkgs.wasm-pack
  ];
  releaseRuntimeInputs = coverageRuntimeInputs ++ [
    pkgs.wasm-pack
  ];
  sdkContractCrates = [
    "xtask"
    "radroots_core"
    "radroots_types"
    "radroots_events"
    "radroots_trade"
    "radroots_identity"
    "radroots_replica_db_schema"
    "radroots_events_codec"
    "radroots_events_codec_wasm"
    "radroots_nostr_connect"
    "radroots_nostr_signer"
  ];
  sdkContractCargoArgs = lib.concatStringsSep " " (map (crate: "-p ${crate}") sdkContractCrates);
  craneLib = (crane.mkLib pkgs).overrideToolchain toolchains.stable;
  commonCraneArgs = {
    inherit version;
    pname = "radroots";
    src = cargoSource;
    strictDeps = true;
    nativeBuildInputs = [
      pkgs.pkg-config
      pkgs.clang
      pkgs.llvmPackages.libclang
    ];
    buildInputs = [
      pkgs.libsodium
    ]
    ++ darwinBuildInputs;
    inherit (sharedEnv)
      CARGO_TERM_COLOR
      LIBCLANG_PATH
      PKG_CONFIG_PATH
      ;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonCraneArgs;
  xtaskPackage = craneLib.buildPackage (
    commonCraneArgs
    // {
      inherit cargoArtifacts;
      pname = "xtask";
      cargoExtraArgs = "-p xtask";
      doCheck = false;
    }
  );
  initGitRepo = ''
    git init -q .
    git config user.email "nix-check@example.invalid"
    git config user.name "nix check"
    git add -A .
  '';
  mkRepoCheck =
    {
      name,
      runtimeInputs,
      command,
      env ? sharedEnv,
      initGit ? false,
      linuxOnly ? false,
    }:
    if linuxOnly && !pkgs.stdenv.isLinux then
      null
    else
      pkgs.runCommand name { nativeBuildInputs = runtimeInputs; } ''
        export HOME="$TMPDIR/home"
        mkdir -p "$HOME"

        cp -R ${repoSource} "$TMPDIR/repo"
        chmod -R u+w "$TMPDIR/repo"
        cd "$TMPDIR/repo"
        export RADROOTS_WORKSPACE_ROOT="$PWD"

        ${exportEnv env}
        ${lib.optionalString initGit initGitRepo}

        ${command}

        touch "$out"
      '';
  ensureRepoRoot = ''
    if [ ! -f Cargo.toml ] || [ ! -f flake.nix ]; then
      echo "run this command from the radroots workspace checkout" >&2
      exit 1
    fi
    export RADROOTS_WORKSPACE_ROOT="$PWD"
  '';
  checkCommand = ''
    cargo check --workspace
  '';
  contractCommand = ''
    ./scripts/ci/guard_committed_ts_artifacts.sh
    ./scripts/ci/guard_no_legacy_identifiers.sh
    cargo check -q ${sdkContractCargoArgs}
    cargo test -q ${sdkContractCargoArgs}
    cargo run -q -p xtask -- sdk validate
    cargo run -q -p xtask -- sdk export-ts --out target/sdk-export-ci
    test -f target/sdk-export-ci/ts/export-manifest.json
  '';
  wasmBuildsCommand = ''
    make build
  '';
  releasePreflightCommand = ''
    ./scripts/ci/release_preflight.sh
  '';
  validateSdkTypescriptCommand = ''
    if [ "$#" -ne 1 ]; then
      echo "usage: validate-sdk-typescript <path-to-sdk-typescript-checkout>" >&2
      exit 1
    fi

    target_dir="$1"
    if [ ! -d "$target_dir" ]; then
      echo "sdk-typescript checkout not found at $target_dir" >&2
      exit 1
    fi

    cd "$target_dir"
    bun install --frozen-lockfile
    bun run typecheck
    bun run build
    bun run test
  '';
  coverageReportCommand = ''
        rm -rf target/sdk-coverage
        mkdir -p target/sdk-coverage
        : > target/sdk-coverage/coverage-report-status.txt

        workspace_crates_file="$(mktemp)"
        required_crates_file="$(mktemp)"
        trap 'rm -f "$workspace_crates_file" "$required_crates_file"' EXIT

        cargo run -q -p xtask -- sdk coverage workspace-crates > "$workspace_crates_file"
        while IFS= read -r crate; do
          [ -n "''${crate}" ] || continue
          safe_crate="''${crate//-/_}"
          run_dir="target/sdk-coverage/''${safe_crate}"
          mkdir -p "''${run_dir}"
          status="ok"

          if ! cargo run -q -p xtask -- sdk coverage run-crate --crate "''${crate}" --out "''${run_dir}"; then
            status="run-failed"
          fi

          if [ "''${status}" = "ok" ] && ! cargo run -q -p xtask -- sdk coverage report \
            --scope "''${crate}" \
            --summary "''${run_dir}/coverage-summary.json" \
            --lcov "''${run_dir}/coverage-lcov.info" \
            --out "''${run_dir}/coverage-gate-summary.json" \
            --fail-under-exec-lines 0 \
            --fail-under-functions 0 \
            --fail-under-regions 0 \
            --fail-under-branches 0; then
            status="report-failed"
          fi

          if [ "''${status}" != "ok" ]; then
            cat > "''${run_dir}/coverage-gate-summary.json" <<EOF
            {
              "scope": "''${crate}",
              "thresholds": {
                "executable_lines": 0,
                "functions": 0,
                "regions": 0,
                "branches": 0,
                "branches_required": false
              },
              "measured": {
                "executable_lines_percent": 0,
                "executable_lines_source": "da",
                "functions_percent": 0,
                "branches_percent": null,
                "branches_available": false,
                "summary_lines_percent": 0,
                "summary_regions_percent": 0
              },
              "counts": {
                "executable_lines": {
                  "covered": 0,
                  "total": 0
                },
                "branches": {
                  "covered": 0,
                  "total": 0
                }
              },
              "result": {
                "pass": false,
                "fail_reasons": [
                  "''${status}"
                ]
              }
            }
    EOF
          fi

          echo "''${crate}:''${status}" >> target/sdk-coverage/coverage-report-status.txt
        done < "$workspace_crates_file"

        cargo run -q -p xtask -- sdk coverage required-crates > "$required_crates_file"
        while IFS= read -r crate; do
          [ -n "''${crate}" ] || continue
          safe_crate="''${crate//-/_}"
          crate_dir="target/sdk-coverage/''${safe_crate}"
          crate_status="$(awk -F: -v crate="''${crate}" '$1 == crate { status = $2 } END { print status }' target/sdk-coverage/coverage-report-status.txt)"

          if [ ! -f "''${crate_dir}/coverage-summary.json" ] || [ ! -f "''${crate_dir}/coverage-lcov.info" ]; then
            fail_reason="missing-coverage-artifacts"
            if [ -n "''${crate_status}" ] && [ "''${crate_status}" != "ok" ]; then
              fail_reason="''${crate_status}"
            fi

            cargo run -q -p xtask -- sdk coverage report-missing \
              --scope "''${crate}-blocking" \
              --out "''${crate_dir}/coverage-gate-blocking.json" \
              --reason "''${fail_reason}"
            continue
          fi

          cargo run -q -p xtask -- sdk coverage report \
            --scope "''${crate}-blocking" \
            --summary "''${crate_dir}/coverage-summary.json" \
            --lcov "''${crate_dir}/coverage-lcov.info" \
            --out "''${crate_dir}/coverage-gate-blocking.json" \
            --policy-gate
        done < "$required_crates_file"
  '';
in
{
  inherit
    cargoLlvmCov
    cargoArtifacts
    checkCommand
    commonCraneArgs
    contractCommand
    coverageEnv
    coverageReportCommand
    craneLib
    ensureRepoRoot
    mkRepoCheck
    releasePreflightCommand
    sdkContractCargoArgs
    sharedEnv
    validateSdkTypescriptCommand
    version
    wasmBuildsCommand
    xtaskPackage
    ;

  exportCoverageEnv = exportEnv coverageEnv;
  exportSharedEnv = exportEnv sharedEnv;

  runtimeInputs = {
    stable = stableRuntimeInputs;
    sync = syncRuntimeInputs;
    coverage = coverageRuntimeInputs;
    release = releaseRuntimeInputs;
    wasm = wasmRuntimeInputs;
  };
}
