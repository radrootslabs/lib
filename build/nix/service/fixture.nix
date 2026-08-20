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
  apps = service.mkServiceApps {
    serviceName = "fixture_service";
    inherit nativeInputs package toolchain;
    binaryName = "fixture-service";
    releaseAcceptanceCommand = ''
      output="$(fixture-service --help)"
      if [ "$output" != "fixture-service" ]; then
        echo "fixture release acceptance observed unexpected output" >&2
        exit 1
      fi
      printf '%s\n' "fixture-service release acceptance"
    '';
  };
  devShell = service.mkServiceDevShell {
    serviceName = "fixture_service";
    inherit nativeInputs toolchain;
  };
  fixtureBuildInfo = {
    serviceVersion = "0.1.0-alpha";
    serviceCommit = "1111111111111111111111111111111111111111";
    libRevision = "2222222222222222222222222222222222222222";
    rustVersion = "1.97.1";
    target = pkgs.stdenv.hostPlatform.rust.rustcTarget;
    featureProfile = "service-host";
    contractVersions = {
      config = 1;
      state = 2;
      admin = 3;
      status = 4;
      provider = 5;
    };
  };
  ociImage =
    if pkgs.stdenv.isLinux then
      service.mkServiceOciImage {
        serviceName = "fixture_service";
        inherit package;
        binaryName = "fixture-service";
        buildInfo = fixtureBuildInfo;
      }
    else
      null;
  ociImageCheck =
    if pkgs.stdenv.isLinux then
      pkgs.runCommand "fixture-service-oci-image"
        {
          nativeBuildInputs = [
            pkgs.coreutils
            pkgs.gnutar
            pkgs.gzip
            pkgs.jq
          ];
        }
        ''
          mkdir image
          tar -xzf ${ociImage} -C image
          config_file="$(jq -er '.[0].Config' image/manifest.json)"
          test -f "image/$config_file"

          jq -e '
            .architecture == "${if pkgs.stdenv.hostPlatform.isAarch64 then "arm64" else "amd64"}" and
            .os == "linux" and
            .created == "1970-01-01T00:00:01+00:00" and
            .config.User == "65532:65532" and
            .config.Entrypoint == ["${package}/bin/fixture-service"] and
            .config.WorkingDir == "/" and
            .config.Env == ["SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"] and
            .config.StopSignal == "SIGTERM" and
            (.config | has("Volumes") | not) and
            .config.Labels == {
              "dev.radroots.build.feature-profile": "service-host",
              "dev.radroots.build.lib-revision": "2222222222222222222222222222222222222222",
              "dev.radroots.build.rust-version": "1.97.1",
              "dev.radroots.build.target": "${pkgs.stdenv.hostPlatform.rust.rustcTarget}",
              "dev.radroots.contract.admin-version": "3",
              "dev.radroots.contract.config-version": "1",
              "dev.radroots.contract.provider-version": "5",
              "dev.radroots.contract.state-version": "2",
              "dev.radroots.contract.status-version": "4",
              "dev.radroots.mount.config": "/etc/radroots/services/fixture_service",
              "dev.radroots.mount.config.mode": "read-only",
              "dev.radroots.mount.credentials": "/etc/radroots/secrets/services/fixture_service",
              "dev.radroots.mount.credentials.mode": "read-only",
              "dev.radroots.mount.runtime": "/run/radroots/services/fixture_service",
              "dev.radroots.mount.runtime.mode": "read-write",
              "dev.radroots.mount.state": "/var/lib/radroots/services/fixture_service",
              "dev.radroots.mount.state.mode": "read-write",
              "dev.radroots.rootfs": "read-only-compatible",
              "org.opencontainers.image.description": "Hardened fixture_service service image",
              "org.opencontainers.image.licenses": "MIT OR Apache-2.0",
              "org.opencontainers.image.revision": "1111111111111111111111111111111111111111",
              "org.opencontainers.image.title": "fixture_service",
              "org.opencontainers.image.version": "0.1.0-alpha"
            }
          ' "image/$config_file"

          jq -e '.[0].RepoTags == ["fixture-service:0.1.0-alpha"]' image/manifest.json
          jq -e '([.[0].Layers | length] | .[0] >= 1 and .[0] <= 2)' image/manifest.json
          jq -er '.[0].Layers[]' image/manifest.json | while IFS= read -r layer; do
            tar -tf "image/$layer"
          done | sort -u > image-entries

          grep -E '/bin/fixture-service$' image-entries
          if grep -E '(^|/)(bin/(ba)?sh|bin/nix|bin/cargo|bin/rustc|Cargo.toml|src/)' image-entries; then
            echo "fixture OCI image contains a development or shell payload" >&2
            exit 1
          fi

          touch "$out"
        ''
    else
      null;
  appSmoke = pkgs.runCommand "fixture-service-app-smoke" { } ''
    test "$(${apps.default.program} --help)" = "fixture-service"
    test "$(${apps.release-acceptance.program})" = "fixture-service release acceptance"
    touch "$out"
  '';
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
    extraChecks = {
      app-smoke = appSmoke;
      inherit smoke;
    }
    // lib.optionalAttrs pkgs.stdenv.isLinux {
      oci-image = ociImageCheck;
    };
  };
  outputs = service.mkServiceOutputs {
    serviceName = "fixture_service";
    inherit nativeInputs package;
    inherit apps checks;
    devShells.default = devShell;
    extraPackages = lib.optionalAttrs pkgs.stdenv.isLinux {
      oci = ociImage;
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
  appArgs = {
    serviceName = "fixture_service";
    inherit nativeInputs package toolchain;
    binaryName = "fixture-service";
    releaseAcceptanceCommand = "fixture-service --help";
  };
  invalidAppResults = [
    (builtins.tryEval (
      (service.mkServiceApps (appArgs // { serviceName = "../fixture"; })).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (appArgs // { package = "not-a-derivation"; })).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (appArgs // { binaryName = "fixture service"; })).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (appArgs // { toolchain = "not-a-derivation"; })).default.program
    ))
    (builtins.tryEval ((service.mkServiceApps (appArgs // { nativeInputs = { }; })).default.program))
    (builtins.tryEval (
      (service.mkServiceApps (
        appArgs
        // {
          nativeInputs = service.mkNativeInputs { nativeBuildInputs = [ "not-a-derivation" ]; };
        }
      )).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (
        appArgs
        // {
          nativeInputs = service.mkNativeInputs { environment.VALUE = 1; };
        }
      )).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (
        appArgs
        // {
          nativeInputs = service.mkNativeInputs { environment."INVALID-NAME" = "value"; };
        }
      )).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (
        appArgs
        // {
          nativeInputs = service.mkNativeInputs { environment.PATH = "/tmp"; };
        }
      )).default.program
    ))
    (builtins.tryEval (
      (service.mkServiceApps (appArgs // { releaseAcceptanceCommand = ""; })).default.program
    ))
  ];
  devShellArgs = {
    serviceName = "fixture_service";
    inherit nativeInputs toolchain;
  };
  invalidDevShellResults = [
    (builtins.tryEval (
      (service.mkServiceDevShell (devShellArgs // { serviceName = "../fixture"; })).drvPath
    ))
    (builtins.tryEval (
      (service.mkServiceDevShell (devShellArgs // { toolchain = "not-a-derivation"; })).drvPath
    ))
    (builtins.tryEval ((service.mkServiceDevShell (devShellArgs // { nativeInputs = { }; })).drvPath))
    (builtins.tryEval (
      (service.mkServiceDevShell (
        devShellArgs
        // {
          nativeInputs = service.mkNativeInputs { buildInputs = [ "not-a-derivation" ]; };
        }
      )).drvPath
    ))
    (builtins.tryEval (
      (service.mkServiceDevShell (
        devShellArgs
        // {
          nativeInputs = service.mkNativeInputs { environment.VALUE = 1; };
        }
      )).drvPath
    ))
    (builtins.tryEval (
      (service.mkServiceDevShell (
        devShellArgs
        // {
          nativeInputs = service.mkNativeInputs { environment."INVALID-NAME" = "value"; };
        }
      )).drvPath
    ))
    (builtins.tryEval (
      (service.mkServiceDevShell (
        devShellArgs
        // {
          nativeInputs = service.mkNativeInputs { environment.RUSTC = "/tmp/rustc"; };
        }
      )).drvPath
    ))
  ];
  ociArgs = {
    serviceName = "fixture_service";
    inherit package;
    binaryName = "fixture-service";
    buildInfo = fixtureBuildInfo;
  };
  maximumOciResult =
    if pkgs.stdenv.isLinux then
      builtins.tryEval (
        (service.mkServiceOciImage (
          ociArgs
          // {
            serviceName = lib.concatStrings (lib.replicate 128 "a");
            binaryName = lib.concatStrings (lib.replicate 128 "b");
            buildInfo = fixtureBuildInfo // {
              serviceVersion = lib.concatStrings (lib.replicate 128 "1");
              contractVersions = lib.mapAttrs (_: _: 4294967295) fixtureBuildInfo.contractVersions;
            };
          }
        )).outPath
      )
    else
      {
        success = true;
        value = null;
      };
  invalidOciResults = lib.optionals pkgs.stdenv.isLinux [
    (builtins.tryEval (
      (service.mkServiceOciImage (ociArgs // { serviceName = "../fixture"; })).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (ociArgs // { package = "not-a-derivation"; })).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (ociArgs // { binaryName = "fixture service"; })).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          serviceName = lib.concatStrings (lib.replicate 129 "a");
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          binaryName = lib.concatStrings (lib.replicate 129 "b");
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            serviceVersion = "bad value";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            serviceVersion = lib.concatStrings (lib.replicate 129 "1");
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            serviceCommit = "ABCDEF";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            libRevision = "bad";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            rustVersion = "stable";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            target = "x86_64-unknown-linux-musl";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            featureProfile = "development";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            extra = "unexpected";
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            contractVersions = fixtureBuildInfo.contractVersions // {
              config = 0;
            };
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            contractVersions = fixtureBuildInfo.contractVersions // {
              config = 4294967296;
            };
          };
        }
      )).outPath
    ))
    (builtins.tryEval (
      (service.mkServiceOciImage (
        ociArgs
        // {
          buildInfo = fixtureBuildInfo // {
            contractVersions = builtins.removeAttrs fixtureBuildInfo.contractVersions [ "provider" ];
          };
        }
      )).outPath
    ))
  ];
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
assert (pkgs.stdenv.isLinux -> outputs.packages.oci == ociImage);
assert (
  pkgs.stdenv.isLinux
  ->
    ociImage.meta.platforms == [
      "aarch64-linux"
      "x86_64-linux"
    ]
);
assert (!pkgs.stdenv.isLinux -> !(builtins.hasAttr "oci" outputs.packages));
assert outputs.checks == checks;
assert
  builtins.attrNames checks == [
    "app-smoke"
    "check"
    "clippy"
    "config"
    "docs"
    "fmt"
    "integration"
  ]
  ++ lib.optionals pkgs.stdenv.isLinux [
    "oci-image"
  ]
  ++ [
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
assert checks.app-smoke == appSmoke;
assert checks.smoke == smoke;
assert outputs.apps == apps;
assert
  builtins.attrNames apps == [
    "default"
    "release-acceptance"
  ];
assert apps.default.program == "${package}/bin/fixture-service";
assert lib.hasSuffix "/bin/fixture_service-release-acceptance" apps.release-acceptance.program;
assert outputs.devShells.default == devShell;
assert devShell.name == "fixture_service-dev-shell";
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
assert lib.all (result: result.success == false) invalidAppResults;
assert lib.all (result: result.success == false) invalidDevShellResults;
assert maximumOciResult.success;
assert lib.all (result: result.success == false) invalidOciResults;
assert (
  !pkgs.stdenv.isLinux
  -> (builtins.tryEval ((service.mkServiceOciImage ociArgs).outPath)).success == false
);
{
  inherit outputs;
}
