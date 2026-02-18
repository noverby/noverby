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

  # Disable systemd-networkd-wait-online.service — the upstream systemd binary
  # tries to talk to networkd via varlink/D-Bus, but the Rust networkd doesn't
  # implement those interfaces yet.  Without this, the oneshot service hangs
  # for 90s (DefaultTimeoutStartSec) blocking the thread pool and preventing
  # idle services (gettys) from starting.
  systemd.services.systemd-networkd-wait-online.enable = lib.mkForce false;

  # Enable systemd-resolved for DNS resolution (tests the Rust resolved)
  # NOTE: Disabled for now — enabling resolved adds units that cause the
  # dependency graph to stall (likely alias handling issue in PID 1).
  # Re-enable once PID 1 properly handles unit aliases.
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
