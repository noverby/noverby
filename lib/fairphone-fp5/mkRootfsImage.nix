# Builds an ext4 image containing the NixOS system that can be flashed to the
# `userdata` partition using fastboot.
#
# Replicated from https://github.com/gian-reto/nixos-fairphone-fp5/blob/main/flake.nix
#
# Parameters:
#   nixosConfig - a NixOS system configuration
#   pkgs        - nixpkgs package set (must be aarch64-linux)
#
# Returns: a derivation producing an ext4 filesystem image.
{
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
}
