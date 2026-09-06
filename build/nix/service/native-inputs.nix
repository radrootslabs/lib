{ lib }:
{
  nativeBuildInputs ? [ ],
  buildInputs ? [ ],
  environment ? { },
}:
assert lib.assertMsg (builtins.isList nativeBuildInputs) "nativeBuildInputs must be a list";
assert lib.assertMsg (builtins.isList buildInputs) "buildInputs must be a list";
assert lib.assertMsg (builtins.isAttrs environment) "environment must be an attribute set";
assert lib.assertMsg (lib.all (name: builtins.match "^[A-Za-z_][A-Za-z0-9_]*$" name != null) (
  builtins.attrNames environment
)) "environment names must be shell identifiers";
assert lib.assertMsg (lib.all (
  name: builtins.match ".*(CREDENTIAL|PASSWORD|PRIVATE_KEY|SECRET|TOKEN).*" (lib.toUpper name) == null
) (builtins.attrNames environment)) "environment names must not identify secret material";
assert lib.assertMsg (lib.all (
  entry:
  builtins.isAttrs entry
  &&
    builtins.attrNames entry == [
      "classification"
      "value"
    ]
  && entry.classification == "nonsecret"
  && builtins.isString entry.value
  && builtins.stringLength entry.value <= 4096
) (builtins.attrValues environment)) "environment values must be bounded typed nonsecret values";
{
  schema = "radroots.service.native-inputs.v1";
  nativeBuildInputs = lib.unique nativeBuildInputs;
  buildInputs = lib.unique buildInputs;
  environment = lib.mapAttrs (_: entry: entry.value) environment;
  environmentContract = environment;
}
