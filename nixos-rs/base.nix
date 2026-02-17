({modulesPath, ...}: {
  imports = ["${modulesPath}/profiles/qemu-guest.nix"];

  networking.useDHCP = false;

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
    nixos = {
      isNormalUser = true;
      extraGroups = ["wheel"];
      password = "nixos";
    };
  };

  services.getty.autologinUser = "nixos";
})
