{
  config,
  inputs,
  lib,
  ...
}: let
  isNixos = x: x ? config.system.build.toplevel;

  standardConfigs =
    lib.filterAttrs (_: cfg: !(isNixos cfg)) config.nixosConfigurations;
in {
  outputs.colmena =
    {
      meta = {
        nixpkgs = import inputs.nixpkgs {
          system = "x86_64-linux";
          config.allowUnfree = true;
        };
        nodeNixpkgs = builtins.mapAttrs (_: cfg:
          import inputs.nixpkgs {
            inherit (cfg) system;
            config.allowUnfree = true;
          })
        standardConfigs;
        nodeSpecialArgs = builtins.mapAttrs (name: cfg:
          {
            inherit inputs;
            hostname = name;
          }
          // (cfg.specialArgs or {}))
        standardConfigs;
      };
    }
    // builtins.mapAttrs (name: cfg: {
      deployment = {
        targetHost = "${name}.overby.me";
        targetUser = "noverby";
      };
      imports =
        [
          config.propagationModule
          ({flake, ...}: {_module.args = {inherit (flake) inputs';};})
        ]
        ++ (cfg.modules or []);
    })
    standardConfigs;
}
