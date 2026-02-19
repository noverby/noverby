{
  config,
  lib,
  inputs,
  ...
}: let
  inherit (lib) mkOption mkMerge mapAttrs;
  inherit (lib.types) lazyAttrsOf unspecified;

  # Create an extended lib with builtin functions
  extendedLib = lib.extend (_: _: {
    inherit (builtins) toJSON fromJSON toFile toString readDir filterSource;
  });
in {
  options = {
    devenvConfigurations = mkOption {
      type = lazyAttrsOf unspecified;
      default = {};
      description = "Devenv configurations to export as devShells";
    };
  };

  config = mkMerge [
    {
      perSystem = {pkgs, ...}: {
        devShells = mapAttrs (_: cfg:
          inputs.devenv.lib.mkShell {
            inherit inputs pkgs;
            lib = extendedLib;
            modules = [
              cfg
            ];
          })
        config.devenvConfigurations;
      };
    }

    {nixDirPathAttrs = ["devenvConfigurations"];}
  ];
}
