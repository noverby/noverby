{
  pkgs,
  stateVersion,
  ...
}: {
  # Nix
  nix = {
    settings = {
      max-jobs = 100;
      trusted-users = ["root" "noverby"];
      experimental-features = "nix-command flakes";
    };
    extraOptions = ''
      min-free = ${toString (30 * 1024 * 1024 * 1024)}
      max-free = ${toString (40 * 1024 * 1024 * 1024)}
    '';
  };

  # System
  system = {
    inherit stateVersion;
    extraSystemBuilderCmds = "ln -s ${./.} $out/full-config";
  };

  # Console
  console = {
    keyMap = "us-acentos";
    font = "ter-132n";
    packages = [pkgs.terminus_font];
  };

  # Bootloader
  boot = {
    loader = {
      timeout = 1;
      systemd-boot.enable = true;
      efi.canTouchEfiVariables = true;
    };
    # Silent boot
    plymouth.enable = true;
    consoleLogLevel = 0;
    initrd.verbose = false;
    kernelParams = ["quiet" "splash" "rd.systemd.show_status=false" "rd.udev.log_level=3" "udev.log_priority=3" "boot.shell_on_fail" "i915.fastboot=1"];
    kernelModules = ["v4l2loopback"];
    kernelPackages = pkgs.linuxPackages_6_10;
    extraModulePackages = [pkgs.linuxPackages_6_10.v4l2loopback];
  };

  # Network
  networking = {
    hostName = "gravitas";
    networkmanager = {
      enable = true;
    };
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

  # Fonts
  fonts.packages = with pkgs;
    [
      (nerdfonts.override {fonts = ["FiraCode" "DroidSansMono"];})
    ]
    ++ [fira roboto roboto-slab meslo-lgs-nf cascadia-code];

  # Audio
  hardware.pulseaudio.enable = false;
  security.rtkit.enable = true;

  # Graphics
  hardware.graphics = {
    enable = true;
  };

  # Virtualisation
  virtualisation = {
    docker.enable = true;
    libvirtd.enable = true;
    waydroid.enable = true;
  };

  # Security
  security.pam.services = {
    gdm.enableGnomeKeyring = true;
    login.fprintAuth = false;
    gdm-fingerprint = with pkgs; {
      text = ''
        auth       required                    pam_shells.so
        auth       requisite                   pam_nologin.so
        auth       requisite                   pam_faillock.so      preauth
        auth       required                    ${fprintd}/lib/security/pam_fprintd.so
        auth       optional                    pam_permit.so
        auth       required                    pam_env.so
        auth       [success=ok default=1]      ${gdm}/lib/security/pam_gdm.so
        auth       optional                    ${gnome-keyring}/lib/security/pam_gnome_keyring.so

        account    include                     login

        password   required                    pam_deny.so

        session    include                     login
        session    optional                    ${gnome-keyring}/lib/security/pam_gnome_keyring.so auto_start
      '';
    };
  };
  programs.seahorse.enable = true;

  # Packages
  environment = {
    systemPackages = with pkgs; [
      gnome-boxes
      helix
      python3
      powertop
      gjs
      glib.dev
      nix-alien
      tailspin
    ];
    sessionVariables = {
      PAGER = "tspin";
      SYSTEMD_PAGERSECURE = "1";
    };
  };
  programs = {
    # Run unpatched binaries
    nix-ld.enable = true;
    # Unbraved Brave
    # https://github.com/MulesGaming/brave-debullshitinator
    chromium = {
      enable = true;
      extraOpts = {
        BraveRewardsDisabled = true;
        BraveWalletDisabled = true;
        BraveVPNDisabled = 1;
        BraveAIChatEnabled = false;
        IPFSEnabled = false;
        PasswordManagerEnabled = false;
      };
    };
  };

  # Users
  environment.profiles = ["$HOME/.local"];
  users.users.noverby = {
    shell = "${pkgs.nushell}/bin/nu";
    isNormalUser = true;
    description = "Niclas Overby";
    extraGroups = ["networkmanager" "wheel" "docker" "libvirtd"];
  };

  # Services
  services = {
    resolved = {
      enable = true;
      extraConfig = ''
        [Resolve]
        DNS=45.90.28.0#5e65f2.dns.nextdns.io
        DNS=2a07:a8c0::#5e65f2.dns.nextdns.io
        DNS=45.90.30.0#5e65f2.dns.nextdns.io
        DNS=2a07:a8c1::#5e65f2.dns.nextdns.io
        DNSOverTLS=yes
      '';
    };
    printing = {
      enable = true;
      drivers = with pkgs; [hplip hplipWithPlugin];
    };
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
    };
    udev.extraRules = ''
      # XReal

      # Rule for USB devices
      SUBSYSTEM=="usb", ACTION=="add", ATTR{idVendor}=="3318", ATTR{idProduct}=="0424|0428|0432", MODE="0666"

      # Rule for Input devices (such as eventX)
      SUBSYSTEM=="input", KERNEL=="event[0-9]*", ATTRS{idVendor}=="3318", ATTRS{idProduct}=="0424|0428|0432", MODE="0666"

      # Rule for Sound devices (pcmCxDx and controlCx)
      SUBSYSTEM=="sound", KERNEL=="pcmC[0-9]D[0-9]p", ATTRS{idVendor}=="3318", ATTRS{idProduct}=="0424|0428|0432", MODE="0666"
      SUBSYSTEM=="sound", KERNEL=="controlC[0-9]", ATTRS{idVendor}=="3318", ATTRS{idProduct}=="0424|0428|0432", MODE="0666"

      # Rule for HID Devices (hidraw)
      SUBSYSTEM=="hidraw", KERNEL=="hidraw[0-9]*", ATTRS{idVendor}=="3318", ATTRS{idProduct}=="0424|0428|0432", MODE="0666"

      # Rule for HID Devices (hiddev)
      KERNEL=="hiddev[0-9]*", SUBSYSTEM=="usb", ATTRS{idVendor}=="3318", ATTRS{idProduct}=="0424|0428|0432", MODE="0666"
    '';
  };

  # Systemd
  systemd.services = {
    powertop = {
      enable = true;
      description = "Powertop tunings";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = "yes";
        ExecStart = "${pkgs.powertop}/bin/powertop --auto-tune";
      };
      wantedBy = ["multi-user.target"];
    };
  };

  # Cosmic
  services = {
    desktopManager.cosmic.enable = true;
    displayManager.cosmic-greeter.enable = false;
  };
}
