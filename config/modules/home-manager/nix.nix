{
  pkgs,
  lib,
  ...
}: {
  nix = {
    package = lib.mkDefault pkgs.nix;
    settings = {
      allow-import-from-derivation = true;
    };
  };
}
