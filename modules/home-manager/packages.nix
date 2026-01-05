{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    # General apps
    #bitwarden
    slack
    fragments
    evince
    bitwarden-desktop
    mpv
    onlyoffice-desktopeditors
    dconf-editor
    rclone
    gnome-network-displays
    gnome-system-monitor
    file-roller
    wireplumber
    gnome-disk-utility
    firefoxpwa
    cheese
    pavucontrol
    kooha
    rustdesk-flutter

    # System tools
    killall
    uutils-coreutils-noprefix
    xorg.xkill
    lsof
    wl-clipboard
    skim
    #waypipe
    wl-color-picker
    cryptsetup

    # Network tools
    xh
    wget
    whois
    openssl
    gping
    bandwhich
    rustscan
    unixtools.route

    # Hardware tools
    acpi
    util-linux
    pciutils
    lshw
    usbutils
    solaar # Logitech Unifying Receiver

    # File tools
    helix
    file
    unixtools.xxd
    fd
    tre
    hexyl
    git-filter-repo
    dust
    ripgrep
    ripgrep-all
    tokei
    zip
    unzip
    p7zip
    uutils-diffutils
    ast-grep
    diffoscope
    jless
    television
    yq-go # Needed by prettybat

    # Container tools
    distrobox
    bubblewrap
    appimage-run
    cloud-hypervisor
    simg2img

    # General dev
    lazyjj
    glab
    granted

    # System dev
    #lldb
    gdb
    cling # C++ repl
    evcxr # Rust repl
    lurk
    tracexec
    llvmPackages.bintools
    binwalk
    hyperfine
    inferno # Flamegraph svg generator
    flamelens # Flamegraph cli viewer
    #darling

    # Nix dev
    nix-du
    nix-diff-rs
    devenv
    nix-prefetch-git
    nix-fast-build
    nix-init
    comma
    nurl
    pkgs.nxv

    # Media tools
    imagemagick
    oxipng
    gimp3

    # Very serious tools
    genact
    fortune-kind
  ];
}
