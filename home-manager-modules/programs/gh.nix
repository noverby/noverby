{
  lib,
  pkgs,
  ...
}: {
  programs.gh.enable = lib.mkDefault pkgs.stdenv.hostPlatform.isx86_64;
}
