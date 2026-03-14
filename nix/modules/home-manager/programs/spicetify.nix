{
  inputs,
  pkgs,
  lib,
  ...
}: {
  programs.spicetify = lib.mkIf pkgs.stdenv.hostPlatform.isx86_64 (let
    spicePkgs = inputs.spicetify-nix.legacyPackages.${pkgs.stdenv.system};
  in {
    enable = true;
    theme = spicePkgs.themes.catppuccin;
    colorScheme = "mocha";
    enabledCustomApps = [
      spicePkgs.apps.ncsVisualizer
    ];
  });
}
