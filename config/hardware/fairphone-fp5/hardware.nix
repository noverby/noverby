# Fairphone 5 hardware module: kernel, firmware, device tree, filesystems,
# serial consoles, and automatic root filesystem expansion.
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.nixos-fairphone-fp5;
in {
  options.nixos-fairphone-fp5 = {
    enable = lib.mkEnableOption "Fairphone 5 hardware support";

    serial = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Enable USB serial console (ttyGS0) for debugging.";
      };

      verbose = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Enable verbose kernel and systemd logging for debugging.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    nixpkgs.hostPlatform = lib.mkDefault "aarch64-linux";

    hardware = {
      deviceTree = {
        enable = true;
        name = "qcom/qcm6490-fairphone-fp5.dtb";
      };

      enableAllFirmware = true;
      firmware = [pkgs.firmware-fairphone-fp5];
      # Qualcomm firmware must be uncompressed.
      firmwareCompression = "none";
    };

    boot = {
      kernelPackages = pkgs.linuxPackagesFor pkgs.kernel-fairphone-fp5;

      initrd = {
        enable = true;
        # PostmarketOS kernel only has CONFIG_RD_GZIP=y.
        compressor = "gzip";

        # Kernel modules required in initramfs for device boot.
        # See: https://gitlab.postmarketos.org/postmarketOS/pmaports/-/blob/master/device/testing/device-fairphone-fp5/modules-initfs
        availableKernelModules =
          [
            "fsa4480" # USB-C audio switch
            "goodix_berlin_core" # Touchscreen core driver
            "goodix_berlin_spi" # Touchscreen SPI interface
            "msm"
            "panel-raydium-rm692e5" # Display panel driver
            "ptn36502" # USB-C redriver
            "spi-geni-qcom" # Qualcomm SPI controller
          ]
          ++ lib.optionals cfg.serial.enable [
            "g_serial" # USB serial gadget (loaded only for debugging)
          ];

        # Disable default modules (like ahci) that don't exist in this kernel.
        includeDefaultModules = false;
        systemd.enable = false;
      };

      # Disable GRUB — we use Android boot image format.
      loader.grub.enable = false;

      # On first boot, register the contents of the initial Nix store.
      postBootCommands = ''
        if [ -f /nix-path-registration ]; then
          set -euo pipefail
          set -x

          ${config.nix.package.out}/bin/nix-store --load-db < /nix-path-registration

          touch /etc/NIXOS
          ${config.nix.package.out}/bin/nix-env -p /nix/var/nix/profiles/system --set /run/current-system

          if [ -d /nix/var/nix/profiles/per-user ]; then
            for profile_dir in /nix/var/nix/profiles/per-user/*; do
              if [ -d "$profile_dir" ]; then
                username=$(basename "$profile_dir")
                echo "Fixing ownership of $profile_dir for user $username"
                chown -R "''${username}:users" "$profile_dir"
              fi
            done
          fi

          rm -f /nix-path-registration
        fi
      '';

      kernelParams =
        lib.mkAfter
        (
          ["loglevel=4"]
          ++ lib.optionals cfg.serial.enable [
            "systemd.log_target=console"
            "console=ttyGS0,115200"
          ]
          ++ [
            # Hardware UART serial console.
            "console=ttyMSM0,115200"
            # Framebuffer console — listed last so it becomes /dev/console.
            "console=tty1"
          ]
          ++ lib.optionals cfg.serial.verbose [
            "ignore_loglevel"
            "systemd.log_level=debug"
          ]
        );
    };

    # Root filesystem.
    fileSystems."/" = {
      device = "/dev/disk/by-label/nixos";
      fsType = "ext4";
    };

    console.earlySetup = true;

    # Serial gettys, A/B slot management, and first-boot resize service.
    systemd.services = {
      # Mark the current A/B boot slot as successful so the bootloader
      # does not exhaust its retry counter and fall back to fastboot.
      mark-boot-successful = {
        description = "Mark current A/B slot as boot-successful";
        wantedBy = ["multi-user.target"];
        after = ["local-fs.target"];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStart = "${pkgs.systemd}/bin/bootctl mark-boot-successful";
        };
      };

      "serial-getty@ttyGS0" = lib.mkIf cfg.serial.enable {
        enable = true;
        wantedBy = ["multi-user.target"];
        serviceConfig.Restart = "always";
      };

      "serial-getty@ttyMSM0" = {
        enable = true;
        wantedBy = ["multi-user.target"];
        serviceConfig.Restart = "always";
      };

      # Automatically resize root filesystem to fill the entire partition on
      # first boot.  The flashed ext4 image is sized to fit only the initial
      # rootfs contents, while the userdata partition is much larger.
      resize-rootfs = {
        description = "Resize root filesystem to fill partition";
        wantedBy = ["local-fs.target"];
        after = ["local-fs.target"];
        before = ["systemd-user-sessions.service"];

        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };

        path = with pkgs; [e2fsprogs gawk util-linux];

        script = ''
          MARKER="/var/lib/rootfs-resized"

          if [ -f "$MARKER" ]; then
            echo "Root filesystem already resized, skipping..."
            exit 0
          fi

          ROOT_DEV=$(findmnt -n -o SOURCE /)
          if [ -z "$ROOT_DEV" ]; then
            echo "ERROR: Could not determine root device"
            exit 1
          fi

          FS_SIZE=$(dumpe2fs -h "$ROOT_DEV" 2>/dev/null | grep -E "^Block count:" | awk '{print $3}')
          BLOCK_SIZE=$(dumpe2fs -h "$ROOT_DEV" 2>/dev/null | grep -E "^Block size:" | awk '{print $3}')

          if [ -z "$FS_SIZE" ] || [ -z "$BLOCK_SIZE" ]; then
            echo "ERROR: Could not determine filesystem size"
            exit 1
          fi

          FS_SIZE_BYTES=$((FS_SIZE * BLOCK_SIZE))
          PART_SIZE=$(blockdev --getsize64 "$ROOT_DEV")
          SIZE_DIFF=$((PART_SIZE - FS_SIZE_BYTES))
          TOLERANCE=$((PART_SIZE / 100))

          if [ $SIZE_DIFF -gt $TOLERANCE ]; then
            echo "Expanding filesystem to fill partition..."
            if resize2fs "$ROOT_DEV"; then
              echo "Successfully resized root filesystem!"
              mkdir -p "$(dirname "$MARKER")"
              touch "$MARKER"
            else
              echo "ERROR: Failed to resize filesystem"
              exit 1
            fi
          else
            echo "Filesystem already at maximum size"
            mkdir -p "$(dirname "$MARKER")"
            touch "$MARKER"
          fi
        '';
      };
    };
  };
}
