{
  src,
  lib,
  config,
  ...
}: let
  inherit (lib) mkOption filterAttrs hasSuffix mapAttrs' removeSuffix;
  inherit (lib.types) lazyAttrsOf unspecified;
  secretsDir = src + /config/secrets;
  dirEntries = builtins.readDir secretsDir;
  ageFiles = filterAttrs (name: _: hasSuffix ".age" name) dirEntries;
in {
  options.secrets = mkOption {
    type = lazyAttrsOf unspecified;
    default = {};
    description = "Age-encrypted secret file paths and public key metadata";
  };

  config = {
    secrets =
      mapAttrs' (name: _: {
        name = removeSuffix ".age" name;
        value = secretsDir + "/${name}";
      })
      ageFiles
      // {
        publicKeys = import (secretsDir + /publicKeys.nix);
      };

    outputs = {inherit (config) secrets;};
  };
}
