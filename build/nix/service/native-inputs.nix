{ lib }:
{
  nativeBuildInputs ? [ ],
  buildInputs ? [ ],
  environment ? { },
}:
assert lib.assertMsg (builtins.isList nativeBuildInputs) "nativeBuildInputs must be a list";
assert lib.assertMsg (builtins.isList buildInputs) "buildInputs must be a list";
assert lib.assertMsg (builtins.isAttrs environment) "environment must be an attribute set";
{
  nativeBuildInputs = lib.unique nativeBuildInputs;
  buildInputs = lib.unique buildInputs;
  inherit environment;
}
