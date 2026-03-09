{
  lib,
  pkgs,
  ...
}: {
  programs.obs-studio = lib.mkIf pkgs.stdenv.hostPlatform.isx86_64 {
    enable = true;
    plugins = with pkgs.obs-studio-plugins; [
      obs-3d-effect
    ];
  };
}
