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
# All builtins are merged into lib at the flake level (see flake.nix) by
# extending nixpkgs lib before importing flakelight.  This means every
# flakelight module receives a lib that includes lib.readDir, lib.fromJSON,
# lib.unsafeDiscardStringContext, etc. without needing builtins.
{
  config,
  lib,
  ...
}: let
  libDir = config.nixDir + "/lib";
  hasLibDir = config.nixDir != null && lib.pathExists libDir;

  resolve = v:
    if lib.isFunction v
    then v lib
    else v;

  # Recursively discover and import .nix files from a directory.
  importLibDir = dir: let
    entries = lib.readDir dir;
    names = lib.attrNames entries;

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
        if lib.pathExists (path + "/default.nix")
        then resolve (import path)
        else importLibDir path
      else {};
  in
    lib.foldl' (acc: name: acc // (processEntry name)) {} names;

  discovered =
    if hasLibDir
    then importLibDir libDir
    else {};

  # Separate perSystemLib attrs from pure lib attrs.
  mergedPerSystemLib = discovered.perSystemLib or {};
  mergedLib = removeAttrs discovered ["perSystemLib"];
in {
  lib = lib.mkForce mergedLib;
  perSystemLib = mergedPerSystemLib;

  # Extend pkgs.lib with all builtins so that code receiving pkgs.lib
  # (callPackage arguments, overlays, etc.) also has lib.readDir,
  # lib.fromJSON, lib.unsafeDiscardStringContext and friends.
  withOverlays = [
    (_: prev: {
      lib = prev.lib.extend (_: prev': builtins // prev');
    })
  ];
}
