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
  outputs = service.mkServiceOutputs {
    serviceName = "fixture_service";
    inherit nativeInputs package;
    checks = {
      inherit smoke;
    };
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
assert outputs.checks.smoke == smoke;
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
{
  inherit outputs;
  check = smoke;
}
