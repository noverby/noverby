{
  config,
  lib,
  inputs,
  ...
}: let
  inherit (lib) mkOption mkMerge mapAttrs;
  inherit (lib.types) lazyAttrsOf unspecified;
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
        devShells = mapAttrs (name: cfg:
          inputs.devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [cfg];
          })
        config.devenvConfigurations;
      };
    }

    {nixDirPathAttrs = ["devenvConfigurations"];}
  ];
}
