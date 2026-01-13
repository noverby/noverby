{pkgs, ...}: {
  boot = {
    loader = {
      timeout = 3;
      grub = {
        enable = true;
        device = "nodev";
        efiSupport = true;
      };
      efi.canTouchEfiVariables = true;
    };
    plymouth.enable = true;
    consoleLogLevel = 0;
    initrd = {
      verbose = false;
      systemd.enable = true;
    };
    kernelParams = [
      "boot.shell_on_fail"
      "loglevel=3"
      "plymouth.use-simpledrm"
      "quiet"
      "rd.systemd.show_status=false"
      "rd.udev.log_level=3"
      "splash"
      "udev.log_priority=3"
    ];
    kernelModules = ["v4l2loopback"];
    kernelPackages = pkgs.linuxPackages_zen;
    extraModulePackages = [pkgs.linuxPackages_zen.v4l2loopback];
    binfmt = {
      emulatedSystems = ["aarch64-linux"];
      # Needed for Docker emulation
      preferStaticEmulators = true;
    };
  };
}
