{
  src,
  lib,
  ...
}: let
  inherit (lib) filterAttrs hasSuffix mapAttrs' removeSuffix;
  usersDir = src + /config/users;
  dirEntries = builtins.readDir usersDir;
  nixFiles = filterAttrs (name: _: hasSuffix ".nix" name) dirEntries;
in {
  outputs.users =
    mapAttrs' (name: _: {
      name = removeSuffix ".nix" name;
      value = usersDir + "/${name}";
    })
    nixFiles;
}
