# Auto-discovers lib functions from nixDir/lib/ directory.
# Each .nix file can be an attrset or a function taking lib, returning an attrset.
# Results are merged flat into the lib flake output.
# Uses mkForce to override nixDir's raw auto-discovery with resolved values.
{
  lib,
  config,
  flakelight,
  ...
}: let
  libDir = config.nixDir + "/lib";
  hasLibDir = config.nixDir != null && builtins.pathExists libDir;
  imported =
    if hasLibDir
    then flakelight.importDir libDir
    else {};
  resolve = v:
    if builtins.isFunction v
    then v lib
    else v;
  mergedLib = builtins.foldl' (acc: v: acc // (resolve v)) {} (builtins.attrValues imported);
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
