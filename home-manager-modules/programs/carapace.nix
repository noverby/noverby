{
  lib,
  pkgs,
  ...
}: {
  programs.carapace = {
    enable = lib.mkDefault pkgs.stdenv.hostPlatform.isx86_64;
  };
}
