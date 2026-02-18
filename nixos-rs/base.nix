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

  # Enable systemd-networkd for network management (tests the Rust networkd)
  networking = {
    useNetworkd = true;
    useDHCP = false;
    # Let networkd handle DHCP per-interface via its .network files
  };

  # Configure networkd to DHCP on all ethernet interfaces
  systemd.network = {
    enable = true;
    networks."10-ethernet" = {
      matchConfig.Name = "en* eth*";
      networkConfig = {
        DHCP = "ipv4";
        IPv6AcceptRA = true;
      };
      dhcpV4Config = {
        UseDNS = true;
        UseRoutes = true;
      };
    };
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
