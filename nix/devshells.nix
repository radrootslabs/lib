{ common, pkgs, toolchains }:
let
  defaultHook = ''
    ${common.exportSharedEnv}
    export PATH=${toolchains.stable}/bin:$PATH
  '';
  coverageHook = ''
    ${common.exportCoverageEnv}
    export PATH=${toolchains.stable}/bin:${toolchains.coverage}/bin:$PATH
  '';
in
{
  default = pkgs.mkShell {
    packages = common.runtimeInputs.wasm ++ [
      pkgs.cargo-llvm-cov
    ];
    shellHook = defaultHook;
  };

  coverage = pkgs.mkShell {
    packages = common.runtimeInputs.release;
    shellHook = coverageHook;
  };

  release = pkgs.mkShell {
    packages = common.runtimeInputs.release;
    shellHook = coverageHook;
  };
}
