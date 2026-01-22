{
  pkgs,
  lib,
  inputs,
  stateVersion,
  src,
  ...
}: let
  usersPath = src + /nix/modules/home-manager/users;
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
    useGlobalPkgs = true;
    useUserPackages = true;
    extraSpecialArgs = {
      inherit inputs pkgs stateVersion src;
    };
  };
}
