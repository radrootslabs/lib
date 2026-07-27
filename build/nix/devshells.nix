{
  common,
  pkgs,
  toolchains,
}:
let
  defaultHook = ''
    ${common.exportSharedEnv}
    export PATH=${toolchains.stable}/bin:$PATH
  '';
  coverageHook = ''
    ${common.exportCoverageEnv}
    export PATH=${toolchains.stable}/bin:${toolchains.coverage}/bin:$PATH
  '';
  decoderSecurityHook = ''
    ${common.exportSharedEnv}
    export PATH=${toolchains.fuzz}/bin:${toolchains.stable}/bin:$PATH
  '';
in
{
  default = pkgs.mkShell {
    packages = common.runtimeInputs.stable ++ [
      common.cargoLlvmCov
    ];
    shellHook = defaultHook;
  };

  coverage = pkgs.mkShell {
    packages = common.runtimeInputs.release;
    shellHook = coverageHook;
  };

  decoder-security = pkgs.mkShell {
    packages = common.runtimeInputs.decoderSecurity;
    shellHook = decoderSecurityHook;
  };

  release = pkgs.mkShell {
    packages = common.runtimeInputs.release;
    shellHook = coverageHook;
  };
}
