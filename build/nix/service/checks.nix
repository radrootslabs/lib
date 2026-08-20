{
  crane,
  lib,
  pkgs,
}:
{
  serviceName,
  toolchain,
  source,
  cargoLock,
  nativeInputs,
  package,
  hooks,
  extraChecks ? { },
}:
assert lib.assertMsg (
  builtins.isString serviceName && builtins.match "^[a-z][a-z0-9_]*$" serviceName != null
) "serviceName must be a lowercase snake-case identifier";
assert lib.assertMsg (lib.isDerivation toolchain) "toolchain must be a derivation";
assert lib.assertMsg (builtins.pathExists source) "source must exist";
assert lib.assertMsg (builtins.pathExists cargoLock) "cargoLock must exist";
assert lib.assertMsg (
  builtins.isAttrs nativeInputs
  && builtins.isList (nativeInputs.nativeBuildInputs or null)
  && builtins.isList (nativeInputs.buildInputs or null)
  && builtins.isAttrs (nativeInputs.environment or null)
) "nativeInputs must come from mkNativeInputs";
assert lib.assertMsg (lib.all (name: !(builtins.hasAttr name nativeInputs.environment)) [
  "CARGO_PROFILE"
  "RUSTDOCFLAGS"
  "RUSTFLAGS"
]) "nativeInputs.environment must not weaken the standard check policy";
assert lib.assertMsg (lib.isDerivation package) "package must be a derivation";
assert lib.assertMsg (builtins.isAttrs hooks) "hooks must be an attribute set";
assert lib.assertMsg (
  builtins.attrNames hooks == [
    "config"
    "integration"
    "source-lock"
    "sqlx"
  ]
) "hooks must provide exactly config, integration, source-lock, and sqlx";
assert lib.assertMsg (lib.all (name: lib.isDerivation hooks.${name}) (
  builtins.attrNames hooks
)) "every service-specific hook must be a derivation";
assert lib.assertMsg (builtins.isAttrs extraChecks) "extraChecks must be an attribute set";
assert lib.assertMsg (lib.all
  (name: builtins.match "^[a-z][a-z0-9-]*$" name != null && lib.isDerivation extraChecks.${name})
  (builtins.attrNames extraChecks)
) "extraChecks must contain lowercase check names bound to derivations";
let
  standardNames = [
    "check"
    "clippy"
    "config"
    "docs"
    "fmt"
    "integration"
    "package"
    "source-lock"
    "sqlx"
    "test"
  ];
in
assert lib.assertMsg (
  builtins.length (
    builtins.attrNames (builtins.intersectAttrs extraChecks (lib.genAttrs standardNames (_: null)))
  ) == 0
) "extraChecks must not replace a standard check";
let
  craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
  commonArgs = {
    pname = "${serviceName}-checks";
    version = "1";
    src = craneLib.cleanCargoSource source;
    inherit cargoLock;
    strictDeps = true;
    nativeBuildInputs = nativeInputs.nativeBuildInputs;
    buildInputs = nativeInputs.buildInputs;
    env = nativeInputs.environment;
    doCheck = false;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
  mkCargoCheck =
    name: command: extraArgs:
    craneLib.mkCargoDerivation (
      commonArgs
      // extraArgs
      // {
        inherit cargoArtifacts;
        pname = "${serviceName}-${name}";
        buildPhaseCargoCommand = command;
        installPhaseCommand = "mkdir -p $out";
      }
    );
  standardChecks = {
    fmt = craneLib.cargoFmt (
      commonArgs
      // {
        pname = "${serviceName}-fmt";
      }
    );
    check = mkCargoCheck "check" "cargo check --workspace --all-targets --locked" { };
    test = mkCargoCheck "test" "cargo test --workspace --all-targets --locked" { };
    clippy = mkCargoCheck "clippy" "cargo clippy --workspace --all-targets --locked -- -D warnings" { };
    docs = mkCargoCheck "docs" "cargo doc --workspace --no-deps --locked" {
      RUSTDOCFLAGS = "-D warnings";
    };
    sqlx = hooks.sqlx;
    config = hooks.config;
    source-lock = hooks."source-lock";
    package = package;
    integration = hooks.integration;
  };
in
standardChecks // extraChecks
