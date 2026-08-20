{
  lib,
  pkgs,
}:
{
  serviceName,
  package,
  binaryName,
  buildInfo,
}:
assert lib.assertMsg pkgs.stdenv.isLinux "service OCI images require a Linux builder";
assert lib.assertMsg (
  pkgs.stdenv.hostPlatform.isAarch64 || pkgs.stdenv.hostPlatform.isx86_64
) "service OCI images support only aarch64-linux and x86_64-linux";
assert lib.assertMsg (
  builtins.isString serviceName
  && builtins.stringLength serviceName <= 128
  && builtins.match "^[a-z][a-z0-9_]*$" serviceName != null
) "serviceName must be a lowercase snake-case identifier";
assert lib.assertMsg (lib.isDerivation package) "package must be a derivation";
assert lib.assertMsg (
  builtins.isString binaryName
  && builtins.stringLength binaryName <= 128
  && builtins.match "^[a-z][a-z0-9_-]*$" binaryName != null
) "binaryName must be a lowercase Cargo binary identifier";
assert lib.assertMsg (builtins.isAttrs buildInfo) "buildInfo must be an attribute set";
assert lib.assertMsg (
  builtins.attrNames buildInfo == [
    "contractVersions"
    "featureProfile"
    "libRevision"
    "rustVersion"
    "serviceCommit"
    "serviceVersion"
    "target"
  ]
) "buildInfo must contain exactly the governed service build fields";
assert lib.assertMsg (
  builtins.isString buildInfo.serviceVersion
  && builtins.stringLength buildInfo.serviceVersion <= 128
  && builtins.match "^[A-Za-z0-9][A-Za-z0-9_.-]*$" buildInfo.serviceVersion != null
) "buildInfo.serviceVersion must be a bounded image-tag-safe value";
assert lib.assertMsg (
  builtins.isString buildInfo.serviceCommit
  && builtins.match "^[0-9a-f]{40}$" buildInfo.serviceCommit != null
) "buildInfo.serviceCommit must be a full lowercase Git revision";
assert lib.assertMsg (
  builtins.isString buildInfo.libRevision
  && builtins.match "^[0-9a-f]{40}$" buildInfo.libRevision != null
) "buildInfo.libRevision must be a full lowercase Git revision";
assert lib.assertMsg (
  buildInfo.rustVersion == "1.97.1"
) "buildInfo.rustVersion must match the governed Rust toolchain";
assert lib.assertMsg (
  buildInfo.target == pkgs.stdenv.hostPlatform.rust.rustcTarget
) "buildInfo.target must match the package host platform";
assert lib.assertMsg (
  buildInfo.featureProfile == "service-host"
) "buildInfo.featureProfile must select the service-host profile";
assert lib.assertMsg (builtins.isAttrs buildInfo.contractVersions) (
  "buildInfo.contractVersions must be an attribute set"
);
assert lib.assertMsg (
  builtins.attrNames buildInfo.contractVersions == [
    "admin"
    "config"
    "provider"
    "state"
    "status"
  ]
) "buildInfo.contractVersions must contain exactly the governed contract versions";
assert lib.assertMsg (lib.all
  (version: builtins.isInt version && version > 0 && version <= 4294967295)
  (builtins.attrValues buildInfo.contractVersions)
) "every buildInfo contract version must be a positive u32";
let
  imageName = lib.replaceStrings [ "_" ] [ "-" ] serviceName;
  user = "65532:65532";
  mountPaths = [
    "/etc/radroots/services/${serviceName}"
    "/etc/radroots/secrets/services/${serviceName}"
    "/run/radroots/services/${serviceName}"
    "/var/lib/radroots/services/${serviceName}"
  ];
  labels = {
    "dev.radroots.build.feature-profile" = buildInfo.featureProfile;
    "dev.radroots.build.lib-revision" = buildInfo.libRevision;
    "dev.radroots.build.rust-version" = buildInfo.rustVersion;
    "dev.radroots.build.target" = buildInfo.target;
    "dev.radroots.contract.admin-version" = toString buildInfo.contractVersions.admin;
    "dev.radroots.contract.config-version" = toString buildInfo.contractVersions.config;
    "dev.radroots.contract.provider-version" = toString buildInfo.contractVersions.provider;
    "dev.radroots.contract.state-version" = toString buildInfo.contractVersions.state;
    "dev.radroots.contract.status-version" = toString buildInfo.contractVersions.status;
    "dev.radroots.mount.config" = "/etc/radroots/services/${serviceName}";
    "dev.radroots.mount.config.mode" = "read-only";
    "dev.radroots.mount.credentials" = "/etc/radroots/secrets/services/${serviceName}";
    "dev.radroots.mount.credentials.mode" = "read-only";
    "dev.radroots.mount.runtime" = "/run/radroots/services/${serviceName}";
    "dev.radroots.mount.runtime.mode" = "read-write";
    "dev.radroots.mount.state" = "/var/lib/radroots/services/${serviceName}";
    "dev.radroots.mount.state.mode" = "read-write";
    "dev.radroots.rootfs" = "read-only-compatible";
    "org.opencontainers.image.description" = "Hardened ${serviceName} service image";
    "org.opencontainers.image.licenses" = "MIT OR Apache-2.0";
    "org.opencontainers.image.revision" = buildInfo.serviceCommit;
    "org.opencontainers.image.title" = serviceName;
    "org.opencontainers.image.version" = buildInfo.serviceVersion;
  };
in
pkgs.dockerTools.buildLayeredImage {
  name = imageName;
  tag = buildInfo.serviceVersion;
  created = "1970-01-01T00:00:01Z";
  maxLayers = 2;
  contents = [
    package
    pkgs.dockerTools.caCertificates
  ];
  config = {
    User = user;
    Entrypoint = [ "${package}/bin/${binaryName}" ];
    WorkingDir = "/";
    Env = [ "SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt" ];
    StopSignal = "SIGTERM";
    Labels = labels;
  };
  passthru.radrootsServiceOci = {
    schema = "radroots.service-oci.v1";
    schemaVersion = 1;
    inherit
      buildInfo
      imageName
      labels
      mountPaths
      serviceName
      user
      ;
    entrypoint = "${package}/bin/${binaryName}";
  };
  meta = {
    description = "Hardened rootless OCI image for ${serviceName}";
    platforms = [
      "aarch64-linux"
      "x86_64-linux"
    ];
  };
}
