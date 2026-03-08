# Auto-discovers lib functions from nixDir/lib/ directory.
# Each .nix file can be an attrset or a function taking lib, returning an attrset.
# Results are merged flat into the lib flake output.
# Uses mkForce to override nixDir's raw auto-discovery with resolved values.
#
# Discovery rules:
#   - .nix files are imported, resolved (called with lib if a function), and merged.
#   - Directories with default.nix are imported as a single unit.
#   - Directories without default.nix are recursed into.
#   - Files and directories starting with _ are considered private and skipped.
{
  lib,
  config,
  ...
}: let
  libDir = config.nixDir + "/lib";
  hasLibDir = config.nixDir != null && builtins.pathExists libDir;

  resolve = v:
    if builtins.isFunction v
    then v lib
    else v;

  # Recursively discover and import .nix files from a directory.
  importLibDir = dir: let
    entries = builtins.readDir dir;
    names = builtins.attrNames entries;

    processEntry = name: let
      type = entries.${name};
      path = dir + "/${name}";
    in
      if lib.hasPrefix "_" name
      then {}
      else if type == "regular" && lib.hasSuffix ".nix" name
      then resolve (import path)
      else if type == "directory"
      then
        if builtins.pathExists (path + "/default.nix")
        then resolve (import path)
        else importLibDir path
      else {};
  in
    builtins.foldl' (acc: name: acc // (processEntry name)) {} names;

  mergedLib =
    if hasLibDir
    then importLibDir libDir
    else {};
in {
  lib = lib.mkForce mergedLib;

  _module.args = {
    lib = lib.extend (
      _: _: {
        inherit (builtins) toJSON fromJSON toFile toString readDir filterSource;
      }
    );
  };
}
