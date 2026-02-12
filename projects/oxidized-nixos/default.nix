{
  nixosConfigurations.oxidized-nixos = {
    inputs,
    lib,
    ...
  }: {
    system = "x86_64-linux";
    modules = [
      ({modulesPath, ...}: {
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
