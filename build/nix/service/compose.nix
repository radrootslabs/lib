{ lib }:
{
  serviceName,
  package,
  nativeInputs,
  extraPackages ? { },
  checks ? { },
  apps ? { },
  devShells ? { },
}:
assert lib.assertMsg (
  builtins.isString serviceName && builtins.match "^[a-z][a-z0-9_]*$" serviceName != null
) "serviceName must be a lowercase snake-case identifier";
assert lib.assertMsg (lib.isDerivation package) "package must be a derivation";
assert lib.assertMsg (
  builtins.isAttrs nativeInputs
  && builtins.isList (nativeInputs.nativeBuildInputs or null)
  && builtins.isList (nativeInputs.buildInputs or null)
  && builtins.isAttrs (nativeInputs.environment or null)
) "nativeInputs must come from mkNativeInputs";
assert lib.assertMsg (builtins.isAttrs extraPackages) "extraPackages must be an attribute set";
assert lib.assertMsg (
  !(builtins.hasAttr "default" extraPackages)
) "extraPackages must not replace packages.default";
assert lib.assertMsg (builtins.isAttrs checks) "checks must be an attribute set";
assert lib.assertMsg (builtins.isAttrs apps) "apps must be an attribute set";
assert lib.assertMsg (builtins.isAttrs devShells) "devShells must be an attribute set";
{
  inherit
    apps
    checks
    devShells
    nativeInputs
    serviceName
    ;
  packages = {
    default = package;
  }
  // extraPackages;
}
