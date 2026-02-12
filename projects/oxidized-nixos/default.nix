{
  devShells.oxidized-nixos = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };
  nixosConfigurations.oxidized-nixos = {
    inputs,
    lib,
    ...
  }: {
    system = "x86_64-linux";
    modules = [
      ({
        modulesPath,
        pkgs,
        lib,
        ...
      }: {
        imports = ["${modulesPath}/profiles/qemu-guest.nix"];

        system = {
          name = "oxidized";
          stateVersion = "25.11";
        };

        boot = {
          kernelParams = ["init=/nix/var/nix/profiles/system/init"];
          loader.grub.enable = false;
        };

        fileSystems."/" = {
          device = "/dev/disk/by-label/nixos";
          autoResize = true;
          fsType = "ext4";
        };

        users.users = {
          oxidized = {
            isNormalUser = true;
            extraGroups = ["wheel"];
            password = "oxidized";
          };
        };

        system.replaceDependencies.replacements = let
          uutils = pkgs.uutils-coreutils-noprefix;
        in [
          {
            original = pkgs.coreutils;
            replacement = uutils.overrideAttrs {name = "coreutils-9.8";};
          }
          {
            original = pkgs.coreutils-full;
            replacement = uutils.overrideAttrs {name = "coreutils-full-9.8";};
          }
        ];

        services.getty.autologinUser = "oxidized";
        security.sudo.enable = false;
        security.sudo-rs = {
          enable = true;
          wheelNeedsPassword = false;
        };
      })
    ];
  };
}
