({
  modulesPath,
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

  # Disable systemd-networkd-wait-online.service — the upstream C binary tries
  # to talk to networkd via varlink/D-Bus, but the Rust networkd doesn't
  # implement those interfaces yet.  The service currently exits quickly
  # (connection refused), so it doesn't block boot, but keeping it disabled
  # avoids a spurious failure in the activation graph.
  systemd.services.systemd-networkd-wait-online.enable = lib.mkForce false;

  # Enable systemd-resolved for DNS resolution (tests the Rust resolved)
  services.resolved = {
    enable = true;
    dnssec = "allow-downgrade";
    llmnr = "true";
    fallbackDns = ["1.1.1.1" "8.8.8.8"];
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
