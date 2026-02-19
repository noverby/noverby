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

  mkDevenvShell = pkgs: cfg:
    inputs.devenv.lib.mkShell {
      inherit inputs pkgs;
      lib = extendedLib;
      modules = [
        cfg
      ];
    };
in {
  options = {
    devenvConfigurations = mkOption {
      type = lazyAttrsOf unspecified;
      default = {};
      description = "Devenv configurations to export as devShells";
    };

    devenvConfiguration = mkOption {
      type = unspecified;
      default = null;
      description = "Devenv configuration to export as the default devShell";
    };
  };

  config = mkMerge [
    {
      devShells =
        mapAttrs (_: cfg: pkgs: mkDevenvShell pkgs cfg)
        config.devenvConfigurations;
    }

    (lib.mkIf (config.devenvConfiguration != null) {
      devShells.default = pkgs: mkDevenvShell pkgs config.devenvConfiguration;
    })

    {nixDirPathAttrs = ["devenvConfigurations" "devenvConfiguration"];}
  ];
}
