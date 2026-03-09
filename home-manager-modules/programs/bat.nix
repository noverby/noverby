{
  lib,
  pkgs,
  ...
}: {
  programs.bat = {
    enable = true;
    extraPackages = with pkgs.bat-extras;
      [
        batgrep
        batdiff
      ]
      ++ lib.optionals pkgs.stdenv.hostPlatform.isx86_64 [
        prettybat
      ];
  };
}
