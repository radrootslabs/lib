{
  lib,
  pkgs,
}:
{
  serviceName,
  toolchain,
  nativeInputs,
}:
assert lib.assertMsg (
  builtins.isString serviceName && builtins.match "^[a-z][a-z0-9_]*$" serviceName != null
) "serviceName must be a lowercase snake-case identifier";
assert lib.assertMsg (lib.isDerivation toolchain) "toolchain must be a derivation";
assert lib.assertMsg (
  builtins.isAttrs nativeInputs
  && builtins.isList (nativeInputs.nativeBuildInputs or null)
  && builtins.isList (nativeInputs.buildInputs or null)
  && builtins.isAttrs (nativeInputs.environment or null)
) "nativeInputs must come from mkNativeInputs";
assert lib.assertMsg (lib.all lib.isDerivation (
  nativeInputs.nativeBuildInputs ++ nativeInputs.buildInputs
)) "nativeInputs must contain only derivations";
assert lib.assertMsg (lib.all builtins.isString (
  builtins.attrValues nativeInputs.environment
)) "nativeInputs.environment values must be strings";
assert lib.assertMsg (lib.all (name: builtins.match "^[A-Za-z_][A-Za-z0-9_]*$" name != null) (
  builtins.attrNames nativeInputs.environment
)) "nativeInputs.environment names must be shell identifiers";
assert lib.assertMsg (lib.all (name: !(builtins.hasAttr name nativeInputs.environment)) [
  "CARGO"
  "PATH"
  "RUSTC"
  "RUSTC_WRAPPER"
  "RUSTC_WORKSPACE_WRAPPER"
  "RUSTUP_TOOLCHAIN"
]) "nativeInputs.environment must not replace the selected toolchain";
pkgs.mkShell {
  name = "${serviceName}-dev-shell";
  packages = lib.unique ([ toolchain ] ++ nativeInputs.nativeBuildInputs);
  buildInputs = nativeInputs.buildInputs;
  shellHook = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (
      name: value: "export ${name}=${lib.escapeShellArg value}"
    ) nativeInputs.environment
  );
}
