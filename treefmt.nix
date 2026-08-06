{
  projectRootFile = "flake.nix";

  settings.global.excludes = [
    ".direnv/**"
    "contracts/crates/generated/**"
    "target/**"
  ];

  programs.nixfmt.enable = true;
  programs.shfmt.enable = true;
  programs.taplo.enable = true;
}
