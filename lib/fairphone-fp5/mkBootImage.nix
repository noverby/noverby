# Builds the boot image that can be flashed to the `boot` partition using fastboot.
#
# Replicated from https://github.com/gian-reto/nixos-fairphone-fp5/blob/main/flake.nix
#
# Usage:
#   lib.mkBootImage self.nixosConfigurations.phone pkgs;
#
# Parameters:
#   nixosConfig - a NixOS system configuration (e.g. nixosConfigurations.phone)
#   pkgs        - nixpkgs package set (must be aarch64-linux)
#
# Returns: a derivation producing a single boot.img file.
{
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
}
