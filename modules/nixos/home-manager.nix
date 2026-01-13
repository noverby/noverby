{
  pkgs,
  lib,
  inputs,
  stateVersion,
  src,
  ...
}: let
  username = "noverby";
  homeDirectory = "/home/${username}";
  usersPath = src + /modules/users;
  users = builtins.listToAttrs (
    map (
      file: {
        name = lib.removeSuffix ".nix" file;
        value = usersPath + "/${file}";
      }
    ) (builtins.attrNames (builtins.readDir usersPath))
  );
in {
  home-manager = {
    inherit users;
    useGlobalPkgs = false;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs username homeDirectory stateVersion;
    };
  };
}
