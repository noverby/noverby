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

  # systemd-oomd is the real systemd OOM killer binary — it requires
  # D-Bus, full cgroup delegation, and memory.pressure support that
  # systemd-rs doesn't yet provide, so it exits immediately with code 1.
  systemd.oomd.enable = false;

  users.users = {
    nixos = {
      isNormalUser = true;
      extraGroups = ["wheel"];
      password = "nixos";
    };
  };

  services.getty.autologinUser = "nixos";
})
