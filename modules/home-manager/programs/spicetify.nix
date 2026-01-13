{
  inputs,
  pkgs,
  ...
}: {
  programs.spicetify = let
    spicePkgs = inputs.spicetify-nix.legacyPackages.${pkgs.stdenv.system};
  in {
    enable = true;
    theme = spicePkgs.themes.catppuccin;
    colorScheme = "mocha";
  };
}
