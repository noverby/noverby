{
  pkgs,
  stateVersion,
  src,
  lib,
  ...
}: {
  # Nix
  nix = {
    settings = {
      max-jobs = 100;
      trusted-users = ["root" "noverby"];
      experimental-features = "nix-command flakes ca-derivations";
      download-buffer-size = 1024 * 1024 * 1024;
      substituters = [
        "https://nix-community.cachix.org"
      ];
      trusted-public-keys = [
        "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
      ];
    };
    daemonCPUSchedPolicy = "idle";
    daemonIOSchedClass = "idle";
    extraOptions = ''
      min-free = ${toString (30 * 1024 * 1024 * 1024)}
      max-free = ${toString (40 * 1024 * 1024 * 1024)}
    '';
  };

  # Enforce Niceness
  systemd.services.nix-daemon.serviceConfig = {
    Nice = lib.mkForce 15;
    IOSchedulingClass = lib.mkForce "idle";
    IPEgressPriority = 7;
    IPIngressPriority = 7;
  };

  # System
  system = {
    inherit stateVersion;
    # Store copy of all Nix files in /nix/var/nix/profiles/system/full-config
    systemBuilderCommands = let
      nixFiles =
        builtins.filterSource (
          path: type:
            type == "directory" || builtins.match ".*\\.nix$" (baseNameOf path) != null
        )
        src;
    in "ln -s ${nixFiles} $out/full-config";
  };

  # Perlless/Bashless activation
  # system = {
  #   nixos-init.enable = true;
  #   etc.overlay.enable = true;
  # };
  # systemd.sysusers.enable = true;

  # Console
  console = {
    keyMap = "us-acentos";
    font = "ter-132n";
    packages = [pkgs.terminus_font];
    earlySetup = true;
  };

  # Bootloader
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

  # Network
  networking = {
    hostName = "gravitas";
    networkmanager = {
      enable = true;
      dns = "systemd-resolved";
    };
  };
  programs.captive-browser = {
    enable = true;
    interface = "wlp2s0";
  };

  # Locale
  time.timeZone = "Europe/Copenhagen";
  i18n = {
    defaultLocale = "en_DK.UTF-8";
    extraLocaleSettings = {
      LC_ADDRESS = "da_DK.UTF-8";
      LC_IDENTIFICATION = "da_DK.UTF-8";
      LC_MEASUREMENT = "da_DK.UTF-8";
      LC_MONETARY = "da_DK.UTF-8";
      LC_NAME = "da_DK.UTF-8";
      LC_NUMERIC = "da_DK.UTF-8";
      LC_PAPER = "da_DK.UTF-8";
      LC_TELEPHONE = "da_DK.UTF-8";
      LC_TIME = "da_DK.UTF-8";
    };
  };

  # Audio
  services.pulseaudio.enable = false;
  security.rtkit.enable = true;

  # Hardware
  hardware = {
    graphics.enable = true;
    logitech.wireless = {
      enable = true;
      enableGraphical = true;
    };
    amdgpu.opencl.enable = true;
  };
  zramSwap.enable = true;

  # Virtualisation
  virtualisation = {
    docker = {
      enable = true;
      daemon.settings = {
        # runtimes = {
        #   youki = {
        #     path = "${pkgs.youki}/bin/youki";
        #   };
        # };
        # default-runtime = "youki";
      };
    };
    libvirtd.enable = true;
    waydroid.enable = true;
  };

  # Fonts
  fonts.packages = with pkgs; [nerd-fonts.fira-code nerd-fonts.droid-sans-mono fira roboto roboto-slab meslo-lgs-nf cascadia-code];

  # Packages
  environment = {
    systemPackages = with pkgs; [
      helix
      tailspin
    ];
    sessionVariables = {
      PAGER = "tspin";
      SYSTEMD_PAGERSECURE = "1";
      NIXOS_OZONE_WL = "1";
    };
  };
  programs = {
    wireshark.enable = true;
    # Run unpatched binaries
    nix-ld.enable = true;
    niri.enable = true;
  };
  security.sudo-rs.enable = true;

  # Users
  environment.profiles = ["$HOME/.local"];
  users.users.noverby = {
    shell = pkgs.nushell;
    isNormalUser = true;
    description = "Niclas Overby";
    extraGroups = ["networkmanager" "wheel" "docker" "libvirtd" "wireshark"];
  };

  # Services
  services = {
    avahi.enable = false;
    resolved = {
      enable = true;
      extraConfig = ''
        [Resolve]
        DNS=94.140.14.49#fb52a727.d.adguard-dns.com 100.100.100.100
        DNSOverTLS=yes
        MulticastDNS=resolve
      '';
    };
    printing.enable = true;
    openssh.enable = true;
    flatpak.enable = true;
    fwupd.enable = true;
    pipewire = {
      enable = true;
      alsa.enable = true;
      alsa.support32Bit = true;
      pulse.enable = true;
    };
    xserver = {
      enable = true;
      excludePackages = [pkgs.xterm];
      xkb = {
        layout = "us";
        variant = "altgr-intl";
      };
      videoDrivers = ["amdgpu" "modesetting"];
    };
    ollama = {
      enable = false;
      package = pkgs.ollama-rocm;
      rocmOverrideGfx = "11.0.2";
    };
    tailscale.enable = true;
  };
}
