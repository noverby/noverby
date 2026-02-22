{
  src,
  lib,
  ...
}: let
  inherit (lib) filterAttrs hasSuffix mapAttrs' removeSuffix;
  secretsDir = src + /config/secrets;
  dirEntries = builtins.readDir secretsDir;
  ageFiles = filterAttrs (name: _: hasSuffix ".age" name) dirEntries;
in {
  outputs.secrets =
    mapAttrs' (name: _: {
      name = removeSuffix ".age" name;
      value = secretsDir + "/${name}";
    })
    ageFiles
    // {
      publicKeys = import (secretsDir + /publicKeys.nix);
    };
}
