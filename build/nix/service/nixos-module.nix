{ lib }:
{
  serviceName,
  binaryName,
  packageFor,
  commandForInstance,
  stopTimeoutSeconds ? 30,
  addressFamilies ? [
    "AF_UNIX"
    "AF_INET"
    "AF_INET6"
  ],
}:
assert lib.assertMsg (
  builtins.isString serviceName
  && builtins.stringLength serviceName <= 128
  && builtins.match "^[a-z][a-z0-9_]*$" serviceName != null
) "serviceName must be a bounded lowercase snake-case identifier";
assert lib.assertMsg (
  builtins.isString binaryName
  && builtins.stringLength binaryName <= 128
  && builtins.match "^[a-z][a-z0-9_-]*$" binaryName != null
) "binaryName must be a lowercase Cargo binary identifier";
assert lib.assertMsg (builtins.isFunction packageFor) "packageFor must be a function";
assert lib.assertMsg (builtins.isFunction commandForInstance)
  "commandForInstance must be a function";
assert lib.assertMsg (
  builtins.isInt stopTimeoutSeconds && stopTimeoutSeconds > 0 && stopTimeoutSeconds <= 86400
) "stopTimeoutSeconds must be between 1 and 86400";
assert lib.assertMsg (
  builtins.isList addressFamilies
  && addressFamilies != [ ]
  && builtins.length addressFamilies <= 3
  && lib.all (
    family:
    builtins.elem family [
      "AF_UNIX"
      "AF_INET"
      "AF_INET6"
    ]
  ) addressFamilies
  && builtins.length (lib.unique addressFamilies) == builtins.length addressFamilies
) "addressFamilies must be a unique nonempty subset of the governed families";
{
  config,
  pkgs,
  ...
}:
let
  optionPath = [
    "services"
    "radroots"
    serviceName
  ];
  cfg = lib.getAttrFromPath optionPath config;
  instanceNames = builtins.attrNames cfg.instances;
  systemUser = "radroots-${serviceName}";
  validInstanceName =
    name:
    builtins.isString name
    && builtins.stringLength name <= 128
    && builtins.match "^[a-z0-9][a-z0-9_-]*[a-z0-9]$|^[a-z0-9]$" name != null;
  unitName = instanceName: "radroots-${serviceName}-${instanceName}";
  validUnitName = instanceName: builtins.stringLength (unitName instanceName) <= 247;
  validCredentialName =
    name:
    builtins.stringLength name <= 128 && builtins.match "^[A-Za-z0-9][A-Za-z0-9_.-]*$" name != null;
  validCredentialPath =
    path:
    builtins.isString path
    && builtins.stringLength path <= 4096
    && lib.hasPrefix "/" path
    && path != "/nix/store"
    && !(lib.hasPrefix "/nix/store/" path)
    && builtins.match "^[^:\n]+$" path != null;
  validCredentials =
    instance:
    builtins.length (builtins.attrNames instance.credentials) <= 32
    && lib.all (name: validCredentialName name && validCredentialPath instance.credentials.${name}) (
      builtins.attrNames instance.credentials
    );
  commandFor = instanceName: commandForInstance instanceName;
  validCommand =
    instanceName:
    let
      command = commandFor instanceName;
    in
    builtins.isList command
    && builtins.length command <= 64
    && lib.all (
      argument:
      builtins.isString argument
      && builtins.stringLength argument <= 4096
      && builtins.match "^[^\n]*$" argument != null
    ) command;
  package = cfg.package;
  assertions = [
    {
      assertion = cfg.enable == (instanceNames != [ ]);
      message = "the service must be enabled with at least one instance and disabled without instances";
    }
    {
      assertion = builtins.length instanceNames <= 64;
      message = "a service may define at most 64 instances";
    }
    {
      assertion = lib.isDerivation package;
      message = "the service package must be a derivation";
    }
    {
      assertion =
        cfg.adminGroup == null
        || (
          builtins.stringLength cfg.adminGroup <= 128
          && builtins.match "^[a-z_][a-z0-9_-]*$" cfg.adminGroup != null
        );
      message = "adminGroup must be a bounded canonical system group name";
    }
  ]
  ++ lib.concatMap (
    instanceName:
    let
      instance = cfg.instances.${instanceName};
    in
    [
      {
        assertion = validInstanceName instanceName;
        message = "every service instance must use a bounded canonical identifier";
      }
      {
        assertion = validUnitName instanceName;
        message = "the derived systemd unit name exceeds its 255-byte limit";
      }
      {
        assertion = validCredentials instance;
        message = "credentials must use bounded names and external absolute source paths";
      }
      {
        assertion = validCommand instanceName;
        message = "the service command must contain at most 64 bounded single-line arguments";
      }
    ]
  ) instanceNames;
  mkService =
    instanceName: instance:
    let
      serviceInstance = "radroots/services/${serviceName}/${instanceName}";
      configurationPath = "/etc/${serviceInstance}/config.toml";
      credentialBindings = lib.mapAttrsToList (name: path: "${name}:${path}") instance.credentials;
      serviceConfig = {
        Type = "simple";
        User = systemUser;
        Group = if cfg.adminGroup == null then systemUser else cfg.adminGroup;
        DynamicUser = true;
        ExecStart = lib.escapeShellArgs ([ "${package}/bin/${binaryName}" ] ++ commandFor instanceName);
        WorkingDirectory = "/";
        Restart = "on-failure";
        RestartSec = "5s";
        TimeoutStopSec = "${toString stopTimeoutSeconds}s";
        KillSignal = "SIGTERM";
        KillMode = "control-group";
        SendSIGKILL = true;
        UMask = "0077";
        ConfigurationDirectory = serviceInstance;
        ConfigurationDirectoryMode = "0500";
        StateDirectory = serviceInstance;
        StateDirectoryMode = "0700";
        CacheDirectory = serviceInstance;
        CacheDirectoryMode = "0700";
        RuntimeDirectory = serviceInstance;
        RuntimeDirectoryMode = if cfg.adminGroup == null then "0700" else "0750";
        RuntimeDirectoryPreserve = false;
        BindReadOnlyPaths = [
          "${instance.configurationFile}:${configurationPath}"
        ];
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectHostname = true;
        ProtectClock = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        RestrictAddressFamilies = addressFamilies;
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RemoveIPC = true;
        DevicePolicy = "closed";
        KeyringMode = "private";
        ProcSubset = "pid";
        ProtectProc = "invisible";
        SystemCallArchitectures = "native";
      }
      // lib.optionalAttrs (credentialBindings != [ ]) {
        LoadCredential = credentialBindings;
      };
    in
    lib.nameValuePair (unitName instanceName) {
      description = "Hardened ${serviceName} service instance ${instanceName}";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      restartIfChanged = true;
      stopIfChanged = true;
      inherit serviceConfig;
    };
in
{
  options = lib.setAttrByPath optionPath {
    enable = lib.mkEnableOption "the hardened ${serviceName} service";
    package = lib.mkOption {
      type = lib.types.package;
      default = packageFor pkgs;
      description = "The exact package containing the ${binaryName} service binary.";
    };
    adminGroup = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Optional existing group admitted by the service's local admin policy.";
    };
    instances = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            configurationFile = lib.mkOption {
              type = lib.types.path;
              description = "A non-secret configuration file mounted read-only at the canonical path.";
            };
            credentials = lib.mkOption {
              type = lib.types.attrsOf lib.types.str;
              default = { };
              description = "External absolute credential source paths passed through LoadCredential.";
            };
          };
        }
      );
      default = { };
      description = "Explicit service instances; every declared instance is active when enabled.";
    };
  };

  config = {
    inherit assertions;
    systemd.services = lib.mkIf cfg.enable (lib.mapAttrs' mkService cfg.instances);
  };
}
