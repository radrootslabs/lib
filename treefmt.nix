{
  projectRootFile = "flake.nix";

  settings.global.excludes = [
    ".direnv/**"
    "target/**"
    "crates/*/bindings/**"
  ];

  programs.nixfmt.enable = true;
  programs.shfmt.enable = true;
  programs.taplo.enable = true;
}
