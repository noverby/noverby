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
#
# Also extends lib with commonly needed builtins (fromJSON, toJSON, etc.)
# via _module.args so all flakelight modules (including devenv) receive them.
# We use a separate baseLib parameter to avoid a circular dependency:
# _module.args.lib must not be defined in terms of the module's own lib arg.
{
  config,
  inputs,
  ...
}: let
  # Use nixpkgs lib directly to avoid circular _module.args.lib dependency.
  baseLib = inputs.nixpkgs.lib;

  extendedLib = baseLib.extend (
    _: _: {
      inherit (builtins) toJSON fromJSON toFile toString readDir filterSource;
    }
  );

  libDir = config.nixDir + "/lib";
  hasLibDir = config.nixDir != null && builtins.pathExists libDir;

  resolve = v:
    if builtins.isFunction v
    then v extendedLib
    else v;

  # Recursively discover and import .nix files from a directory.
  importLibDir = dir: let
    entries = builtins.readDir dir;
    names = builtins.attrNames entries;

    processEntry = name: let
      type = entries.${name};
      path = dir + "/${name}";
    in
      if baseLib.hasPrefix "_" name
      then {}
      else if type == "regular" && baseLib.hasSuffix ".nix" name
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
  lib = baseLib.mkForce mergedLib;

  _module.args = {
    lib = extendedLib;
  };
}
