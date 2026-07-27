{
  crane,
  lib,
  pkgs,
  toolchains,
}:
let
  root = ../..;
  cargoToml = builtins.fromTOML (builtins.readFile ../../Cargo.toml);
  version = cargoToml.workspace.package.version;
  darwinBuildInputs = lib.optionals pkgs.stdenv.isDarwin [
    pkgs.libiconv
  ];
  repoSource = lib.sources.cleanSource root;
  cargoSource = lib.fileset.toSource {
    root = root;
    fileset = lib.fileset.intersection (lib.fileset.fromSource repoSource) (
      lib.fileset.unions [
        ../../.cargo
        ../../Cargo.toml
        ../../Cargo.lock
        ../../flake.lock
        ../../CHANGELOG.md
        ../../README
        ../../flake.nix
        ../../build/nix/apps.nix
        ../../build/nix/checks.nix
        ../../build/nix/common.nix
        ../../build/nix/devshells.nix
        ../../build/nix/toolchains.nix
        ../../dto_bindgen.toml
        ../../rust-toolchain.toml
        ../../rust-toolchain-ios.toml
        ../../rust-toolchain-fuzz.toml
        ../../contracts
        ../../crates
        ../../docs/blossom-raster-decoder-security.md
        ../../fuzz
        ../../tools
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
  cargoLlvmCov = pkgs.cargo-llvm-cov.overrideAttrs (old: {
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
      perl
      pkg-config
      python3
    ]
    ++ darwinBuildInputs;
  coverageRuntimeInputs = stableRuntimeInputs ++ [
    toolchains.coverage
    cargoLlvmCov
  ];
  decoderSecurityStableRuntimeInputs = stableRuntimeInputs ++ [
    pkgs.imagemagick
    pkgs.time
  ];
  decoderSecurityFuzzRuntimeInputs = stableRuntimeInputs ++ [
    toolchains.fuzz
    pkgs.cargo-fuzz
  ];
  decoderSecurityIosRuntimeInputs = stableRuntimeInputs ++ [
    toolchains.ios
  ];
  decoderSecurityRuntimeInputs = stableRuntimeInputs ++ [
    toolchains.fuzz
    pkgs.cargo-fuzz
    pkgs.imagemagick
    pkgs.time
  ];
  fuzzCargoDeps = pkgs.rustPlatform.importCargoLock {
    lockFile = ../../fuzz/Cargo.lock;
  };
  releaseRuntimeInputs = coverageRuntimeInputs;
  coreContractCrates = [
    "xtask"
    "radroots_blossom"
    "radroots_core"
    "radroots_event"
    "radroots_trade"
    "radroots_identity"
    "radroots_replica_schema"
    "radroots_event_codec"
    "radroots_event_store"
    "radroots_nostr"
    "radroots_nostr_connect"
    "radroots_nostr_signer"
  ];
  coreContractCargoArgs =
    lib.concatStringsSep " " (map (crate: "-p ${crate}") coreContractCrates)
    + " --features radroots_blossom/raster-decode,radroots_event_codec/serde_json,radroots_event_codec/nostr,radroots_nostr/blossom,radroots_nostr/client,radroots_nostr/codec,radroots_nostr/events";
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
      pkgs.perl
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
      cargoDeps ? null,
    }:
    if linuxOnly && !pkgs.stdenv.isLinux then
      null
    else
      pkgs.runCommand name
        (
          {
            nativeBuildInputs =
              runtimeInputs
              ++ lib.optionals (cargoDeps != null) [
                pkgs.rustPlatform.cargoSetupHook
              ];
          }
          // lib.optionalAttrs (cargoDeps != null) { inherit cargoDeps; }
        )
        ''
            export HOME="$TMPDIR/home"
            mkdir -p "$HOME"

            cp -R ${repoSource} "$TMPDIR/repo"
            chmod -R u+w "$TMPDIR/repo"
          cd "$TMPDIR/repo"
          export RADROOTS_WORKSPACE_ROOT="$PWD"

          ${exportEnv env}
          ${lib.optionalString (cargoDeps != null) "cargoSetupPostUnpackHook"}
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
    cargo check --workspace --all-targets
  '';
  contractCommand = ''
    cargo run -q -p xtask -- hygiene forbidden-identifiers
    cargo check -q ${coreContractCargoArgs}
    cargo test -q ${coreContractCargoArgs}
    cargo run -q -p xtask -- contract validate
  '';
  decoderSecurityStableCommand = ''
    stable_cargo=${toolchains.stable}/bin/cargo
    magick=${pkgs.imagemagick}/bin/magick

    "$stable_cargo" test -p radroots_blossom \
      --no-default-features \
      --features raster-decode,serde \
      --test decoder_security \
      decoder_regression_corpus_executes_every_case

    RADROOTS_INDEPENDENT_RASTER_DECODER=${pkgs.imagemagick}/bin/magick \
      "$stable_cargo" test -p radroots_blossom \
        --no-default-features \
        --features raster-decode,serde \
        --test decoder_security \
        decoder_differential_matches_independent_backend \
        -- --ignored --exact

    test_executable="$($stable_cargo test -p radroots_blossom \
      --no-default-features \
      --features raster-decode,serde \
      --test decoder_security \
      --no-run \
      --message-format=json \
      | jq -r 'select(.profile.test == true and .target.name == "decoder_security") | .executable' \
      | tail -n 1)"
    if [ -z "$test_executable" ] || [ ! -x "$test_executable" ]; then
      echo "failed to resolve decoder_security test executable" >&2
      exit 1
    fi

    ${lib.optionalString pkgs.stdenv.isDarwin ''
      if otool -L "$test_executable" | grep -i 'libwebp'; then
        echo "decoder test executable must not dynamically link libwebp" >&2
        exit 1
      fi
    ''}

    cargo_target_root="''${CARGO_TARGET_DIR:-$PWD/target}"
    mkdir -p "$cargo_target_root"
    resource_root="$(mktemp -d "$cargo_target_root/decoder-security-resource-matrix.XXXXXX")"
    fixture_root="$resource_root/fixtures"
    evidence_root="$resource_root/rss"
    mkdir -p "$fixture_root" "$evidence_root"

    "$magick" -size 5000x4000 xc:'#204060' -strip -colorspace Gray \
      -sampling-factor 1x1 -quality 85 "$fixture_root/jpeg_grayscale.jpg"
    "$magick" -size 5000x4000 xc:'#204060' -strip -type TrueColor \
      -colorspace sRGB -sampling-factor 2x2 -quality 85 "$fixture_root/jpeg_rgb.jpg"
    "$magick" -size 5000x4000 xc:'cmyk(10%,20%,30%,5%)' -strip \
      -colorspace CMYK -type ColorSeparation -sampling-factor 1x1 -quality 85 \
      "$fixture_root/jpeg_cmyk.jpg"
    cp "$fixture_root/jpeg_rgb.jpg" "$fixture_root/jpeg_sof1.jpg"
    perl -0777pi -e 's/\xFF\xC0/\xFF\xC1/ or die "SOF0 marker missing\n"' \
      "$fixture_root/jpeg_sof1.jpg"

    "$magick" -size 5000x4000 xc:'#204060' -strip -type TrueColor -depth 8 \
      "PNG24:$fixture_root/png_rgb.png"
    "$magick" -size 5000x4000 pattern:checkerboard -strip -type Palette -depth 8 \
      -define png:color-type=3 -define png:bit-depth=8 \
      "$fixture_root/png_palette.png"
    "$magick" -size 5000x4000 xc:'rgba(32,64,96,0.5)' -strip -alpha on -depth 8 \
      "PNG32:$fixture_root/png_rgba.png"
    "$magick" -size 5000x4000 xc:'rgba(32,64,96,0.5)' -strip -alpha on -depth 8 \
      -interlace PNG "PNG32:$fixture_root/png_adam7.png"

    "$magick" -size 5000x4000 xc:'#204060' -strip -type TrueColor \
      -define webp:lossless=false -quality 75 "$fixture_root/webp_vp8_rgb.webp"
    "$magick" -size 5000x4000 xc:'#204060' -strip -alpha set -channel A \
      -evaluate set 50% +channel -define webp:lossless=false -quality 75 \
      "$fixture_root/webp_vp8_alpha.webp"
    "$magick" -size 5000x4000 xc:'#204060' -strip -type TrueColor \
      -define webp:lossless=true "$fixture_root/webp_vp8l_rgb.webp"
    "$magick" -size 5000x4000 xc:'#204060' -strip -alpha set -channel A \
      -evaluate set 50% +channel -define webp:lossless=true \
      "$fixture_root/webp_vp8l_alpha.webp"

    "$magick" -size 16384x1 xc:'#204060' -strip -type TrueColor -depth 8 \
      "PNG24:$fixture_root/axis_width_16384.png"
    "$magick" -size 1x16384 xc:'#204060' -strip -type TrueColor -depth 8 \
      "PNG24:$fixture_root/axis_height_16384.png"

    resource_cases='jpeg_grayscale jpeg_rgb jpeg_cmyk jpeg_sof1 png_rgb png_palette png_rgba png_adam7 webp_vp8_rgb webp_vp8_alpha webp_vp8l_rgb webp_vp8l_alpha'
    evidence_file="$resource_root/maximum-rss-kib.tsv"
    printf 'case_id\tmaximum_rss_kib\n' > "$evidence_file"
    for resource_case in $resource_cases; do
      highest_rss_kib=0
      for repetition in 1 2 3; do
        rss_file="$evidence_root/$resource_case.$repetition.rss-kib"
        ${pkgs.time}/bin/time -f '%M' -o "$rss_file" \
          env \
            RADROOTS_DECODER_RESOURCE_CASE="$resource_case" \
            RADROOTS_DECODER_RESOURCE_FIXTURE_ROOT="$fixture_root" \
            "$test_executable" maximum_resource_probe --ignored --exact
        peak_rss_kib="$(tr -d '[:space:]' < "$rss_file")"
        case "$peak_rss_kib" in
          ""|*[!0-9]*)
            echo "invalid peak RSS measurement for $resource_case: $peak_rss_kib" >&2
            exit 1
            ;;
        esac
        if [ "$peak_rss_kib" -gt 131072 ]; then
          echo "$resource_case peak RSS $peak_rss_kib KiB exceeds 131072 KiB" >&2
          exit 1
        fi
        if [ "$peak_rss_kib" -gt "$highest_rss_kib" ]; then
          highest_rss_kib="$peak_rss_kib"
        fi
      done
      printf '%s\t%s\n' "$resource_case" "$highest_rss_kib" >> "$evidence_file"
      echo "$resource_case peak RSS: $highest_rss_kib KiB (limit: 131072 KiB)"
    done

    for axis_case in width_16384 height_16384; do
      env \
        RADROOTS_DECODER_RESOURCE_AXIS_CASE="$axis_case" \
        RADROOTS_DECODER_RESOURCE_FIXTURE_ROOT="$fixture_root" \
        "$test_executable" axis_resource_probe --ignored --exact
    done
    echo "decoder resource evidence: $evidence_file"
  '';
  decoderSecurityIosCommand = ''
    if [ "$(uname -s)" != Darwin ]; then
      echo "the aarch64-apple-ios compile/link lane requires a Darwin host" >&2
      exit 1
    fi

    ios_xcrun=/usr/bin/xcrun
    ios_sdk="$(env -u DEVELOPER_DIR -u SDKROOT "$ios_xcrun" --sdk iphoneos --show-sdk-path)"
    ios_clang="$(env -u DEVELOPER_DIR -u SDKROOT "$ios_xcrun" --sdk iphoneos --find clang)"
    ios_ar="$(env -u DEVELOPER_DIR -u SDKROOT "$ios_xcrun" --sdk iphoneos --find ar)"
    unset DEVELOPER_DIR
    export SDKROOT="$ios_sdk"
    export IPHONEOS_DEPLOYMENT_TARGET=16.0
    export CC_aarch64_apple_ios="$ios_clang"
    export AR_aarch64_apple_ios="$ios_ar"
    export CFLAGS_aarch64_apple_ios="--target=arm64-apple-ios16.0 -isysroot $ios_sdk"
    export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$ios_clang"
    export CARGO_TARGET_AARCH64_APPLE_IOS_RUSTFLAGS="-C link-arg=-isysroot -C link-arg=$ios_sdk -C link-arg=-miphoneos-version-min=16.0"
    export RUSTC=${toolchains.ios}/bin/rustc
    export RUSTDOC=${toolchains.ios}/bin/rustdoc

    ios_cargo=${toolchains.ios}/bin/cargo
    "$ios_cargo" rustc -p radroots_blossom \
      --lib \
      --crate-type staticlib \
      --target aarch64-apple-ios \
      --no-default-features \
      --features raster-decode,serde

    ios_archive="''${CARGO_TARGET_DIR:?}/aarch64-apple-ios/debug/libradroots_blossom.a"
    if [ ! -f "$ios_archive" ]; then
      echo "missing aarch64-apple-ios static archive: $ios_archive" >&2
      exit 1
    fi
    lipo -info "$ios_archive" | grep -F 'arm64' >/dev/null
    archive_members="$(${pkgs.cctools}/bin/ar -t "$ios_archive")"
    printf '%s\n' "$archive_members" | grep -F 'libwebp_sys' >/dev/null
    printf '%s\n' "$archive_members" | grep -F -- '-webp_dec.o' >/dev/null
    printf '%s\n' "$archive_members" | grep -F -- '-vp8_dec.o' >/dev/null
    printf '%s\n' "$archive_members" | grep -F -- '-vp8l_dec.o' >/dev/null
    echo "aarch64-apple-ios static link verified: $ios_archive"
  '';
  decoderSecurityFuzzCommand = ''
    fuzz_cargo=${toolchains.fuzz}/bin/cargo
    fuzz_runner=${pkgs.cargo-fuzz}/bin/cargo-fuzz

    cargo_target_root="''${CARGO_TARGET_DIR:-$TMPDIR/cargo-target}"
    mkdir -p "$cargo_target_root"
    smoke_root="$(mktemp -d "$cargo_target_root/decoder-security-fuzz-smoke.XXXXXX")"
    mkdir -p "$smoke_root/corpus" "$smoke_root/artifacts"
    cp -R fuzz/corpus/. "$smoke_root/corpus/"

    export PATH=${toolchains.fuzz}/bin:${pkgs.cargo-fuzz}/bin:$PATH
    export CARGO="$fuzz_cargo"
    export CARGO_TARGET_DIR="$cargo_target_root"
    for target in publication_jpeg publication_png publication_webp; do
      mkdir -p "$smoke_root/artifacts/$target"
      "$fuzz_runner" run --fuzz-dir fuzz "$target" "$smoke_root/corpus/$target" -- \
        -runs=256 \
        -seed=424242 \
        -max_len=65536 \
        -timeout=5 \
        -rss_limit_mb=2048 \
        -artifact_prefix="$smoke_root/artifacts/$target/"
    done
  '';
  decoderSecurityCommand = decoderSecurityStableCommand + decoderSecurityFuzzCommand;
  releasePreflightCommand = ''
    cargo check -q
    cargo test -q -p xtask
    cargo run -q -p xtask -- contract validate

    required_file="$(mktemp)"
    trap 'rm -f "$required_file"' EXIT
    cargo run -q -p xtask -- coverage required-crates > "$required_file"

    rm -rf target/coverage
    mkdir -p target/coverage

    while IFS= read -r crate; do
      [ -n "$crate" ] || continue
      safe_crate="''${crate//-/_}"
      out_dir="target/coverage/''${safe_crate}"
      mkdir -p "$out_dir"

      cargo run -q -p xtask -- coverage run-crate --crate "$crate" --out "$out_dir"
      cargo run -q -p xtask -- coverage report \
        --scope "$crate" \
        --summary "$out_dir/coverage-summary.json" \
        --lcov "$out_dir/coverage-lcov.info" \
        --out "$out_dir/gate-report.json" \
        --policy-gate
    done < "$required_file"

    cargo run -q -p xtask -- coverage refresh-summary \
      --reports-root target/coverage \
      --out target/coverage/coverage-refresh.tsv \
      --status-out target/coverage/coverage-refresh-status.tsv

    cargo run -q -p xtask -- release preflight
    echo "release preflight complete"
  '';
  coverageReportCommand = ''
        rm -rf target/coverage-report
        mkdir -p target/coverage-report
        : > target/coverage-report/coverage-report-status.txt

        workspace_crates_file="$(mktemp)"
        required_crates_file="$(mktemp)"
        trap 'rm -f "$workspace_crates_file" "$required_crates_file"' EXIT

        cargo run -q -p xtask -- coverage workspace-crates > "$workspace_crates_file"
        while IFS= read -r crate; do
          [ -n "''${crate}" ] || continue
          safe_crate="''${crate//-/_}"
          run_dir="target/coverage-report/''${safe_crate}"
          mkdir -p "''${run_dir}"
          status="ok"

          if ! cargo run -q -p xtask -- coverage run-crate --crate "''${crate}" --out "''${run_dir}"; then
            status="run-failed"
          fi

          if [ "''${status}" = "ok" ] && ! cargo run -q -p xtask -- coverage report \
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

          echo "''${crate}:''${status}" >> target/coverage-report/coverage-report-status.txt
        done < "$workspace_crates_file"

        cargo run -q -p xtask -- coverage required-crates > "$required_crates_file"
        while IFS= read -r crate; do
          [ -n "''${crate}" ] || continue
          safe_crate="''${crate//-/_}"
          crate_dir="target/coverage-report/''${safe_crate}"
          crate_status="$(awk -F: -v crate="''${crate}" '$1 == crate { status = $2 } END { print status }' target/coverage-report/coverage-report-status.txt)"

          if [ ! -f "''${crate_dir}/coverage-summary.json" ] || [ ! -f "''${crate_dir}/coverage-lcov.info" ]; then
            fail_reason="missing-coverage-artifacts"
            if [ -n "''${crate_status}" ] && [ "''${crate_status}" != "ok" ]; then
              fail_reason="''${crate_status}"
            fi

            cargo run -q -p xtask -- coverage report-missing \
              --scope "''${crate}" \
              --out "''${crate_dir}/coverage-gate-blocking.json" \
              --reason "''${fail_reason}"
            continue
          fi

          cargo run -q -p xtask -- coverage report \
            --scope "''${crate}" \
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
    decoderSecurityFuzzCommand
    decoderSecurityIosCommand
    decoderSecurityCommand
    decoderSecurityStableCommand
    fuzzCargoDeps
    mkRepoCheck
    releasePreflightCommand
    coreContractCargoArgs
    sharedEnv
    version
    xtaskPackage
    ;

  exportCoverageEnv = exportEnv coverageEnv;
  exportSharedEnv = exportEnv sharedEnv;

  runtimeInputs = {
    stable = stableRuntimeInputs;
    coverage = coverageRuntimeInputs;
    decoderSecurity = decoderSecurityRuntimeInputs;
    decoderSecurityFuzz = decoderSecurityFuzzRuntimeInputs;
    decoderSecurityIos = decoderSecurityIosRuntimeInputs;
    decoderSecurityStable = decoderSecurityStableRuntimeInputs;
    release = releaseRuntimeInputs;
  };
}
