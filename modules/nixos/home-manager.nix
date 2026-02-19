{
  pkgs,
  lib,
  inputs,
  stateVersion,
  src,
  ...
}: let
  usersPath = src + /modules/home-manager/users;
  users = lib.listToAttrs (
    map (
      file: {
        name = lib.removeSuffix ".nix" file;
        value = usersPath + "/${file}";
      }
    ) (lib.attrNames (lib.readDir usersPath))
  );
in {
  home-manager = {
    inherit users;
    useGlobalPkgs = true;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs stateVersion src;
    };
  };
}
