{
  lib,
  pkgs,
  ...
}: {
  catppuccin = {
    enable = true;
    kvantum.enable = false;
  };
  qt = lib.mkIf pkgs.stdenv.hostPlatform.isx86_64 {
    enable = true;
    style.name = "qtct";
    platformTheme.name = "qtct";
  };
}
