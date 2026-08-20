{
  lib,
  pkgs,
  service,
}:
let
  nativeInputs = service.mkNativeInputs {
    nativeBuildInputs = [ pkgs.coreutils ];
    environment = {
      RADROOTS_SERVICE_FIXTURE = "1";
    };
  };
  package = pkgs.writeShellApplication {
    name = "fixture-service";
    runtimeInputs = nativeInputs.nativeBuildInputs;
    text = ''
      case "''${1:-}" in
        --help)
          echo "fixture-service"
          ;;
        *)
          echo "usage: fixture-service --help" >&2
          exit 2
          ;;
      esac
    '';
  };
  smoke =
    pkgs.runCommand "radroots-service-helper-fixture-smoke"
      {
        nativeBuildInputs = [
          package
          pkgs.gnugrep
        ];
      }
      ''
        fixture-service --help > output
        grep -Fx "fixture-service" output
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
{
  inherit outputs;
  check = smoke;
}
