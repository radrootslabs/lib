{
  crane,
  lib,
  pkgs,
}:
{
  toolchain,
  source,
  cargoLock,
  servicePackage,
  binaryName ? servicePackage,
  nativeInputs,
  releaseProfile ? "release",
}:
assert lib.assertMsg (lib.isDerivation toolchain) "toolchain must be a derivation";
assert lib.assertMsg (builtins.pathExists source) "source must exist";
assert lib.assertMsg (builtins.pathExists cargoLock) "cargoLock must exist";
assert lib.assertMsg (
  builtins.isString servicePackage && builtins.match "^[a-z][a-z0-9_-]*$" servicePackage != null
) "servicePackage must be a lowercase Cargo package identifier";
assert lib.assertMsg (
  builtins.isString binaryName && builtins.match "^[a-z][a-z0-9_-]*$" binaryName != null
) "binaryName must be a lowercase Cargo binary identifier";
assert lib.assertMsg (
  builtins.isString releaseProfile
  && builtins.match "^release(-[a-z0-9][a-z0-9_-]*)?$" releaseProfile != null
) "releaseProfile must be release or a release-prefixed Cargo profile";
assert lib.assertMsg (
  builtins.isAttrs nativeInputs
  && builtins.isList (nativeInputs.nativeBuildInputs or null)
  && builtins.isList (nativeInputs.buildInputs or null)
  && builtins.isAttrs (nativeInputs.environment or null)
) "nativeInputs must come from mkNativeInputs";
assert lib.assertMsg (
  !(builtins.hasAttr "CARGO_PROFILE" nativeInputs.environment)
) "nativeInputs.environment must not replace CARGO_PROFILE";
let
  craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
  cargoExtraArgs = "--locked --package ${servicePackage} --bin ${binaryName}";
  commonArgs = {
    src = craneLib.cleanCargoSource source;
    inherit cargoLock cargoExtraArgs;
    CARGO_PROFILE = releaseProfile;
    strictDeps = true;
    nativeBuildInputs = nativeInputs.nativeBuildInputs;
    buildInputs = nativeInputs.buildInputs;
    env = nativeInputs.environment;
    doCheck = false;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
  }
)
