{pkgs, ...}: {
  home.packages = with pkgs; [
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
    sudo-rs
    killall
    uutils-coreutils-noprefix
    xorg.xkill
    lsof
    wl-clipboard
    skim
    waypipe
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

    # Hardware tools
    acpi
    util-linux
    pciutils
    lshw
    usbutils
    solaar # Logitech Unifying Receiver

    # File tools
    file
    unixtools.xxd
    fd
    tre
    hexyl
    git-filter-repo
    du-dust
    ripgrep
    ripgrep-all
    tokei
    zip
    unzip
    p7zip
    mergiraf
    uutils-diffutils
    delta
    ast-grep
    diffoscope
    jless

    # Container tools
    distrobox
    bubblewrap
    appimage-run
    cloud-hypervisor
    simg2img

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
    glab
    #darling

    # Nix dev
    envy
    nix-tree
    devenv
    nix-prefetch-git
    nix-fast-build
    nix-init
    comma
    nurl

    # Media tools
    imagemagick
    oxipng
    gimp3

    # Very serious tools
    genact
    fortune-kind
  ];
}
