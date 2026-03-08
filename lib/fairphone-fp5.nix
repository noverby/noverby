# Fairphone 5 image builders.
#
# Replicated from https://github.com/gian-reto/nixos-fairphone-fp5/blob/main/flake.nix
# so we don't need the upstream flake as an input.
#
# Usage (in flake outputs):
#   boot-image  = lib.mkBootImage  self.nixosConfigurations.phone pkgs;
#   rootfs-image = lib.mkRootfsImage self.nixosConfigurations.phone pkgs;
{
  # Builds the boot image that can be flashed to the `boot` partition using fastboot.
  #
  # Parameters:
  #   nixosConfig - a NixOS system configuration (e.g. nixosConfigurations.phone)
  #   pkgs        - nixpkgs package set (must be aarch64-linux)
  #
  # Returns: a derivation producing a single boot.img file.
  mkBootImage = nixosConfig: pkgs:
    pkgs.runCommand "boot.img" {
      nativeBuildInputs = [pkgs.android-tools];
    } ''
      # Get paths from NixOS configuration.
      kernelPath="${nixosConfig.config.system.build.kernel}"
      initrdPath="${nixosConfig.config.system.build.initialRamdisk}/initrd"
      initPath="${builtins.unsafeDiscardStringContext nixosConfig.config.system.build.toplevel}/init"

      # Build kernel command line from NixOS config parameters.
      # Add init= parameter to kernel params from config.
      kernelParams="${builtins.toString nixosConfig.config.boot.kernelParams}"
      cmdline="$kernelParams init=$initPath"

      # Concatenate kernel (Image.gz) with device tree blob.
      # The bootloader expects them as a single file.
      echo "Concatenating kernel and DTB..."
      cat "$kernelPath/Image.gz" "$kernelPath/dtbs/qcom/qcm6490-fairphone-fp5.dtb" > Image-with-dtb.gz

      # Build Android boot image using mkbootimg.
      # Parameters based on PostmarketOS deviceinfo.
      echo "Building boot image with mkbootimg..."
      echo "Using cmdline: $cmdline"
      mkbootimg \
        --header_version 2 \
        --kernel Image-with-dtb.gz \
        --ramdisk "$initrdPath" \
        --cmdline "$cmdline" \
        --base 0x00000000 \
        --kernel_offset 0x00008000 \
        --ramdisk_offset 0x01000000 \
        --dtb_offset 0x01f00000 \
        --tags_offset 0x00000100 \
        --pagesize 4096 \
        --dtb "$kernelPath/dtbs/qcom/qcm6490-fairphone-fp5.dtb" \
        -o "$out"

      echo "Boot image created successfully: $out"
      echo "Size: $(stat -c%s "$out") bytes"
    '';

  # Builds an ext4 image containing the NixOS system that can be flashed to the
  # `userdata` partition using fastboot.
  #
  # Parameters:
  #   nixosConfig - a NixOS system configuration
  #   pkgs        - nixpkgs package set (must be aarch64-linux)
  #
  # Returns: a derivation producing an ext4 filesystem image.
  mkRootfsImage = nixosConfig: pkgs:
    pkgs.callPackage "${pkgs.path}/nixos/lib/make-ext4-fs.nix" {
      storePaths = [nixosConfig.config.system.build.toplevel];
      # Don't compress, as firmware needs to be uncompressed.
      compressImage = false;
      # Must match `fileSystems."/".device` label defined in the hardware module.
      volumeLabel = "nixos";
      populateImageCommands = ''
        # Create the profile directory structure.
        mkdir -p ./files/nix/var/nix/profiles

        # Create first-generation NixOS profile and point to our initial toplevel.
        ln -s ${nixosConfig.config.system.build.toplevel} ./files/nix/var/nix/profiles/system-1-link

        # Set "system" to point to first-generation profile.
        ln -s system-1-link ./files/nix/var/nix/profiles/system

        # The Android bootloader appends init=/init to the kernel cmdline, which
        # overrides our init=/nix/var/.../init parameter. Instead of fighting the
        # bootloader, we create the symlink it expects. This symlink is stable and
        # always points to the current generation.
        ln -s /nix/var/nix/profiles/system/init ./files/init
      '';
    };

  # Like mkRootfsImage, but also includes Home Manager activation packages
  # in the image closure so the first boot can activate home-manager profiles.
  #
  # Parameters:
  #   nixosConfig - a NixOS system configuration (with home-manager module)
  #   pkgs        - nixpkgs package set (must be aarch64-linux)
  #
  # Returns: a derivation producing an ext4 filesystem image.
  mkRootfsImageWithHomeManager = nixosConfig: pkgs: let
    # Get all users that have home-manager configurations.
    hmUsers = builtins.attrNames (nixosConfig.config.home-manager.users or {});

    # Collect all home-manager activation packages.
    hmActivationPackages =
      builtins.map
      (user: nixosConfig.config.home-manager.users.${user}.home.activationPackage)
      hmUsers;
  in
    pkgs.callPackage "${pkgs.path}/nixos/lib/make-ext4-fs.nix" {
      storePaths =
        [
          nixosConfig.config.system.build.toplevel
        ]
        ++ hmActivationPackages;
      # Don't compress, as firmware needs to be uncompressed.
      compressImage = false;
      # Must match `fileSystems."/".device` label defined in the hardware module.
      volumeLabel = "nixos";
      populateImageCommands = ''
        # Create the profile directory structure.
        mkdir -p ./files/nix/var/nix/profiles
        mkdir -p ./files/nix/var/nix/profiles/per-user

        # Create first-generation NixOS profile.
        ln -s ${nixosConfig.config.system.build.toplevel} ./files/nix/var/nix/profiles/system-1-link
        # Set "system" to point to first-generation profile.
        ln -s system-1-link ./files/nix/var/nix/profiles/system

        # The bootloader expects /init.
        ln -s /nix/var/nix/profiles/system/init ./files/init

        # Create home-manager profiles for each user.
        ${builtins.concatStringsSep "\n" (builtins.map (user: ''
            # Create profile directory for ${user}.
            mkdir -p ./files/nix/var/nix/profiles/per-user/${user}

            # Create first-generation home-manager profile for ${user}.
            ln -s ${nixosConfig.config.home-manager.users.${user}.home.activationPackage} \
              ./files/nix/var/nix/profiles/per-user/${user}/home-manager-1-link
            # Set home-manager to point to first-generation home profile.
            ln -s home-manager-1-link \
              ./files/nix/var/nix/profiles/per-user/${user}/home-manager

            # Create user's .nix-profile symlink.
            mkdir -p ./files/home/${user}
            ln -s /nix/var/nix/profiles/per-user/${user}/home-manager \
              ./files/home/${user}/.nix-profile
          '')
          hmUsers)}
      '';
    };
}
