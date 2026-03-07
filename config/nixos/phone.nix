# ╔═══════════════════════════════════════════════════════════════════════╗
# ║  PHONE — Fairphone 5                                                ║
# ║                                                                      ║
# ║  NixOS on Fairphone 5 (QCM6490 / Qualcomm SC7280) with COSMIC DE.  ║
# ║  Cross-compiled from x86_64-linux.                                   ║
# ║                                                                      ║
# ║  Based on: https://github.com/gian-reto/nixos-fairphone-fp5         ║
# ╚═══════════════════════════════════════════════════════════════════════╝
#
# Flashing (first time):
#   1. Unlock the bootloader (see PostmarketOS wiki for Fairphone 5)
#   2. Put device into fastboot mode (power off, hold volume-down + power)
#   3. Connect via USB-C
#   4. Build & flash boot image:
#        nix build .#nixosConfigurations.phone.config.system.build.kernel
#   5. Build & flash rootfs image (ext4 for userdata partition)
#
# Subsequent updates (over SSH / on-device):
#   nixos-rebuild switch --flake .#phone --target-host root@<phone-ip>
#
# Note on cross-compilation:
#   This configuration sets nixpkgs.buildPlatform = "x86_64-linux" so that
#   the entire system closure can be built on an x86_64 host.  The upstream
#   nixos-fairphone-fp5 project claims cross-compilation is unsupported;
#   we work around this by:
#     - Ensuring build-time tools (pil-squasher, qmic) are nativeBuildInputs
#     - Using buildPackages.stdenv for the kernel config derivation
#     - Relaxing platform restrictions from aarch64-only to all linux
{
  inputs,
  src,
  lib,
  ...
}: {
  system = "aarch64-linux";

  specialArgs = {
    inherit src inputs lib;
    stateVersion = "25.05";
    hasSecrets = false;
  };

  modules = [
    # ── Fairphone 5 hardware support ──────────────────────────────────
    inputs.self.hardware.fairphone-fp5

    # ── Desktop environment ───────────────────────────────────────────
    inputs.self.desktops.cosmic

    # ── Machine configuration ─────────────────────────────────────────
    ({pkgs, ...}: {
      # ── Cross-compilation ───────────────────────────────────────────
      # Build on x86_64-linux, target aarch64-linux.
      # This is what makes it possible to build the entire phone image
      # from an x86_64 workstation without needing an aarch64 builder.
      nixpkgs.buildPlatform = "x86_64-linux";

      # ── Identity ────────────────────────────────────────────────────
      networking.hostName = "phone";

      # ── Nix settings ────────────────────────────────────────────────
      nix.settings = {
        experimental-features = ["nix-command" "flakes"];
        # When on the device, prefer remote builders to avoid draining
        # battery and running out of memory during local rebuilds.
        # trusted-users = ["root" "noverby"];
      };

      # Disable NixOS manual (saves closure size and hides desktop icon).
      documentation.nixos.enable = false;

      # ── Networking ──────────────────────────────────────────────────
      networking = {
        wireless.iwd.enable = true;
        networkmanager = {
          enable = true;
          wifi.backend = "iwd";
        };
        firewall.enable = true;
      };

      # ── Bluetooth ───────────────────────────────────────────────────
      hardware.bluetooth = {
        enable = true;
        powerOnBoot = true;
      };

      # ── Audio (PipeWire) ────────────────────────────────────────────
      security.rtkit.enable = true;

      # ── Display / GPU ───────────────────────────────────────────────
      hardware.graphics.enable = true;

      # ── Services ────────────────────────────────────────────────────
      services = {
        pipewire = {
          enable = true;
          alsa.enable = true;
          pulse.enable = true;
        };

        openssh = {
          enable = true;
          settings = {
            PermitRootLogin = "prohibit-password";
            PasswordAuthentication = false;
          };
        };

        upower.enable = true;
        thermald.enable = false; # Not applicable on ARM
      };

      # ── Useful packages ─────────────────────────────────────────────
      environment.systemPackages = with pkgs; [
        # System utilities
        htop
        wl-clipboard

        # Networking
        blueman
      ];

      # ── User ────────────────────────────────────────────────────────
      users = {
        mutableUsers = true;

        users.noverby = {
          isNormalUser = true;
          description = "Niclas Overby";
          initialPassword = "changeme";
          extraGroups = [
            "networkmanager"
            "video"
            "wheel"
          ];
          openssh.authorizedKeys.keys = [
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOachAYzBH8Qaorvbck99Fw+v6md3BeVtfL5PJ/byv4Cc"
          ];
        };
      };

      security.sudo.wheelNeedsPassword = false;

      # ── System ──────────────────────────────────────────────────────
      system.stateVersion = "25.05";
    })
  ];
}
