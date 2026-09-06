{
  lib,
  pkgs,
  version,
}:
let
  root = ../..;
  releaseSource = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      ../../Cargo.toml
      ../../Cargo.lock
      ../../CHANGELOG.md
      ../../LICENSE-APACHE
      ../../LICENSE-MIT
      ../../README.md
      ../../contracts
      ../../crates
    ];
  };
  package = pkgs.runCommand "radroots-lib-release-bundle-${version}" { } ''
    install -d "$out/share/radroots-lib"
    cp -R ${releaseSource}/. "$out/share/radroots-lib/"
    cat > "$out/share/radroots-lib/release-bundle.json" <<EOF
    {"artifact":"public_library_workspace_release_bundle","package":"radroots","version":"${version}"}
    EOF
  '';
  inspector = pkgs.writeShellApplication {
    name = "radroots-lib-bundle-inspect";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      set -euo pipefail
      test -f ${package}/share/radroots-lib/Cargo.toml
      test -f ${package}/share/radroots-lib/Cargo.lock
      exec cat ${package}/share/radroots-lib/release-bundle.json
    '';
  };
in
{
  inherit package;
  app = {
    type = "app";
    program = "${inspector}/bin/radroots-lib-bundle-inspect";
    meta.description = "Inspect the installed Radroots Lib release bundle";
  };
  check = pkgs.runCommand "radroots-lib-release-bundle-check" { } ''
    test -f ${package}/share/radroots-lib/Cargo.toml
    test -f ${package}/share/radroots-lib/Cargo.lock
    test -f ${package}/share/radroots-lib/release-bundle.json
    test ! -e ${package}/share/radroots-lib/build/nix/service/fixture-service
    touch "$out"
  '';
}
