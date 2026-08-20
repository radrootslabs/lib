{
  lib,
  pkgs,
  service,
  toolchain,
}:
let
  nativeInputs = service.mkNativeInputs {
    nativeBuildInputs = [ pkgs.coreutils ];
    environment = {
      RADROOTS_SERVICE_FIXTURE = "1";
    };
  };
  fixtureSource = ./fixture-service;
  package = service.mkServicePackage {
    inherit nativeInputs toolchain;
    source = fixtureSource;
    cargoLock = fixtureSource + "/Cargo.lock";
    servicePackage = "fixture-service";
    binaryName = "fixture-service";
    releaseProfile = "release";
  };
  hooks = {
    sqlx = pkgs.runCommand "fixture-service-sqlx" { } ''
      if grep -F "sqlx" ${fixtureSource}/Cargo.toml; then
        echo "fixture unexpectedly acquired a SQLx dependency" >&2
        exit 1
      fi
      touch "$out"
    '';
    config = pkgs.runCommand "fixture-service-config" { } ''
      grep -Fx 'publish = false' ${fixtureSource}/Cargo.toml
      touch "$out"
    '';
    source-lock = pkgs.runCommand "fixture-service-source-lock" { } ''
      grep -Fx 'version = 4' ${fixtureSource}/Cargo.lock
      touch "$out"
    '';
    integration = pkgs.runCommand "fixture-service-integration" { nativeBuildInputs = [ package ]; } ''
      fixture-service --help | grep -Fx "fixture-service"
      touch "$out"
    '';
  };
  smoke =
    pkgs.runCommand "radroots-service-helper-fixture-smoke"
      {
        nativeBuildInputs = [
          package
          pkgs.file
          pkgs.gnugrep
          pkgs.nix
        ];
      }
      ''
        fixture-service --help > output
        grep -Fx "fixture-service" output
        file ${package}/bin/fixture-service > file-type
        if grep -Fi "script" file-type; then
          echo "fixture package installed a source wrapper" >&2
          exit 1
        fi
        test ! -e ${package}/Cargo.toml
        test ! -e ${package}/src
        if nix-store --query --requisites ${package} | grep -Fx ${toolchain}; then
          echo "fixture runtime closure retains the Rust toolchain" >&2
          exit 1
        fi
        touch "$out"
      '';
  checks = service.mkServiceChecks {
    serviceName = "fixture_service";
    inherit
      hooks
      nativeInputs
      package
      toolchain
      ;
    source = fixtureSource;
    cargoLock = fixtureSource + "/Cargo.lock";
    extraChecks.smoke = smoke;
  };
  outputs = service.mkServiceOutputs {
    serviceName = "fixture_service";
    inherit nativeInputs package;
    inherit checks;
    apps.default = {
      type = "app";
      program = "${package}/bin/fixture-service";
    };
    devShells.default = pkgs.mkShell {
      packages = nativeInputs.nativeBuildInputs;
      shellHook = ''
        export RADROOTS_SERVICE_FIXTURE=${lib.escapeShellArg nativeInputs.environment.RADROOTS_SERVICE_FIXTURE}
      '';
    };
  };
  invalidName = builtins.tryEval (
    (service.mkServiceOutputs {
      serviceName = "../fixture";
      inherit nativeInputs package;
    }).packages.default.outPath
  );
  defaultOverride = builtins.tryEval (
    (service.mkServiceOutputs {
      serviceName = "fixture_service";
      inherit nativeInputs package;
      extraPackages.default = package;
    }).packages.default.outPath
  );
  invalidPackage = builtins.tryEval (
    (service.mkServiceOutputs {
      serviceName = "fixture_service";
      inherit nativeInputs;
      package = "not-a-derivation";
    }).packages.default
  );
  invalidNativeInputs = builtins.tryEval (
    (service.mkServiceOutputs {
      serviceName = "fixture_service";
      inherit package;
      nativeInputs = { };
    }).nativeInputs
  );
  invalidServicePackage = builtins.tryEval (
    (service.mkServicePackage {
      inherit nativeInputs toolchain;
      source = fixtureSource;
      cargoLock = fixtureSource + "/Cargo.lock";
      servicePackage = "../fixture";
    }).outPath
  );
  invalidBinaryName = builtins.tryEval (
    (service.mkServicePackage {
      inherit nativeInputs toolchain;
      source = fixtureSource;
      cargoLock = fixtureSource + "/Cargo.lock";
      servicePackage = "fixture-service";
      binaryName = "fixture service";
    }).outPath
  );
  invalidReleaseProfile = builtins.tryEval (
    (service.mkServicePackage {
      inherit nativeInputs toolchain;
      source = fixtureSource;
      cargoLock = fixtureSource + "/Cargo.lock";
      servicePackage = "fixture-service";
      releaseProfile = "dev";
    }).outPath
  );
  profileOverride = builtins.tryEval (
    (service.mkServicePackage {
      inherit toolchain;
      source = fixtureSource;
      cargoLock = fixtureSource + "/Cargo.lock";
      servicePackage = "fixture-service";
      nativeInputs = service.mkNativeInputs {
        environment.CARGO_PROFILE = "dev";
      };
    }).outPath
  );
  checkArgs = {
    serviceName = "fixture_service";
    inherit
      hooks
      nativeInputs
      package
      toolchain
      ;
    source = fixtureSource;
    cargoLock = fixtureSource + "/Cargo.lock";
  };
  invalidHookResults = map (
    hookName:
    builtins.tryEval (
      (service.mkServiceChecks (
        checkArgs
        // {
          hooks = hooks // {
            ${hookName} = "not-a-derivation";
          };
        }
      )).check.outPath
    )
  ) (builtins.attrNames hooks);
  missingHook = builtins.tryEval (
    (service.mkServiceChecks (
      checkArgs
      // {
        hooks = builtins.removeAttrs hooks [ "sqlx" ];
      }
    )).check.outPath
  );
  unexpectedHook = builtins.tryEval (
    (service.mkServiceChecks (
      checkArgs
      // {
        hooks = hooks // {
          other = smoke;
        };
      }
    )).check.outPath
  );
  weakenedPolicyResults =
    map
      (
        variable:
        builtins.tryEval (
          (service.mkServiceChecks (
            checkArgs
            // {
              nativeInputs = service.mkNativeInputs {
                environment.${variable} = "override";
              };
            }
          )).check.outPath
        )
      )
      [
        "CARGO_PROFILE"
        "RUSTDOCFLAGS"
        "RUSTFLAGS"
      ];
  invalidExtraCheck = builtins.tryEval (
    (service.mkServiceChecks (
      checkArgs
      // {
        extraChecks.other = "not-a-derivation";
      }
    )).check.outPath
  );
  standardOverride = builtins.tryEval (
    (service.mkServiceChecks (
      checkArgs
      // {
        extraChecks.test = smoke;
      }
    )).check.outPath
  );
in
assert
  service.supportedSystems == [
    "aarch64-darwin"
    "aarch64-linux"
    "x86_64-darwin"
    "x86_64-linux"
  ];
assert nativeInputs.nativeBuildInputs == [ pkgs.coreutils ];
assert nativeInputs.buildInputs == [ ];
assert nativeInputs.environment.RADROOTS_SERVICE_FIXTURE == "1";
assert outputs.serviceName == "fixture_service";
assert outputs.packages.default == package;
assert outputs.checks == checks;
assert
  builtins.attrNames checks == [
    "check"
    "clippy"
    "config"
    "docs"
    "fmt"
    "integration"
    "package"
    "smoke"
    "source-lock"
    "sqlx"
    "test"
  ];
assert checks.package == package;
assert checks.sqlx == hooks.sqlx;
assert checks.config == hooks.config;
assert checks.source-lock == hooks.source-lock;
assert checks.integration == hooks.integration;
assert checks.smoke == smoke;
assert outputs.apps.default.program == "${package}/bin/fixture-service";
assert outputs.devShells.default != null;
assert invalidName.success == false;
assert defaultOverride.success == false;
assert invalidPackage.success == false;
assert invalidNativeInputs.success == false;
assert invalidServicePackage.success == false;
assert invalidBinaryName.success == false;
assert invalidReleaseProfile.success == false;
assert profileOverride.success == false;
assert lib.all (result: result.success == false) invalidHookResults;
assert missingHook.success == false;
assert unexpectedHook.success == false;
assert lib.all (result: result.success == false) weakenedPolicyResults;
assert invalidExtraCheck.success == false;
assert standardOverride.success == false;
{
  inherit outputs;
}
