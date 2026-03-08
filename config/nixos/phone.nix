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
    stateVersion = "25.11";
    hasSecrets = true;
  };

  modules = [
    # ── Fairphone 5 hardware support ──────────────────────────────────
    inputs.self.hardware.fairphone-fp5
    {
      nixos-fairphone-fp5.serial = {
        enable = false;
        verbose = true;
      };
    }

    # ── Secrets ───────────────────────────────────────────────────────
    inputs.ragenix.nixosModules.default
    ({pkgs, ...}: {
      # Simplified age config for the phone (no FIDO2/Nitrokey needed).
      # Decryption uses the pre-generated SSH host key injected into the
      # rootfs image at build time.  The key is stored age-encrypted in
      # config/secrets/phone-host-key.age and must be decrypted before
      # building the rootfs image:
      #   rage -d -i ~/.ssh/id_ed25519 config/secrets/phone-host-key.age \
      #     -o /tmp/phone-hostkeys/ssh_host_ed25519_key
      # The mkRootfsImage populateImageCommands then copies it into the
      # ext4 image at /etc/ssh/.
      age = {
        ageBin = "${pkgs.rage}/bin/rage";
        identityPaths = ["/etc/ssh/ssh_host_ed25519_key"];
      };
    })

    # ── Desktop environment ───────────────────────────────────────────
    inputs.self.desktops.cosmic

    # ── Machine configuration ─────────────────────────────────────────
    ({pkgs, ...}: {
      # ── Cross-compilation ───────────────────────────────────────────
      # Build on x86_64-linux, target aarch64-linux.
      # This is what makes it possible to build the entire phone image
      # from an x86_64 workstation without needing an aarch64 builder.
      nixpkgs.buildPlatform = "x86_64-linux";

      # Workaround: iniparser has doCheck = true which adds
      # -DBUILD_TESTING:BOOL=TRUE to cmakeFlags, requiring ruby for
      # its test suite.  Ruby is not available when cross-compiling,
      # so we disable the check phase.
      nixpkgs.overlays = [
        (final: prev: {
          iniparser = prev.iniparser.overrideAttrs {
            doCheck = false;
          };
          ibus = prev.ibus.overrideAttrs {
            enableParallelInstalling = false;
          };
          gjs = prev.gjs.overrideAttrs (old: {
            mesonFlags =
              (old.mesonFlags or [])
              ++ [
                "-Dskip_gtk_tests=true"
              ];
          });
          xdg-desktop-portal-cosmic = prev.xdg-desktop-portal-cosmic.overrideAttrs (old: {
            buildInputs = (old.buildInputs or []) ++ [final.glib];
          });
          power-profiles-daemon = prev.power-profiles-daemon.overrideAttrs (old: {
            mesonFlags =
              (old.mesonFlags or [])
              ++ [
                "-Dmanpage=disabled"
                "-Dbashcomp=disabled"
                "-Dzshcomp="
              ];
          });
        })
      ];

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

      # Auto-login: COSMIC greeter has no on-screen keyboard, so we
      # bypass it to land directly on the desktop via touchscreen.
      services.displayManager.autoLogin = {
        enable = true;
        user = "noverby";
      };

      # Pre-configured WiFi network for headless SSH access.
      age.secrets."wifi-concero.nmconnection" = {
        file = inputs.self.secrets.wifi-concero;
        path = "/etc/NetworkManager/system-connections/Concero.nmconnection";
        owner = "root";
        group = "root";
        mode = "0600";
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
      system.stateVersion = "25.11";
    })
  ];
}
