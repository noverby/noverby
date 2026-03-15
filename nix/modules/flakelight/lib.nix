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
# Modules that define `perSystemLib` attrs are automatically routed to
# the perSystemLib option (system-dependent functions taking pkgs).
# All other attrs are merged into the flake's lib output (system-independent).
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
      inherit
        (builtins)
        toJSON
        fromJSON
        toFile
        toString
        readDir
        readFile
        filterSource
        fetchurl
        ;
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

  discovered =
    if hasLibDir
    then importLibDir libDir
    else {};

  # Separate perSystemLib attrs from pure lib attrs.
  mergedPerSystemLib = discovered.perSystemLib or {};
  mergedLib = removeAttrs discovered ["perSystemLib"];

  inherit (builtins) removeAttrs;
in {
  lib = baseLib.mkForce mergedLib;
  perSystemLib = mergedPerSystemLib;

  _module.args = {
    lib = extendedLib;
  };
}
