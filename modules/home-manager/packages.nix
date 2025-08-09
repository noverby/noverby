{pkgs, ...}: {
  home.packages = with pkgs; [
    # Pop
    pop-launcher
    pop-icon-theme
    pop-gtk-theme

    # General apps
    genact
    #bitwarden
    slack
    fragments
    evince
    bitwarden-desktop
    mpv
    onlyoffice-desktopeditors
    gnome-tweaks
    dconf-editor
    rclone
    gnome-network-displays
    gnome-system-monitor
    file-roller
    eyedropper
    wireplumber
    gnome-disk-utility
    firefoxpwa
    cheese

    # System tools
    sudo-rs
    killall
    uutils-coreutils-noprefix
    fortune-kind
    xorg.xkill
    lsof
    wl-clipboard
    fpp
    skim
    pueue
    waypipe

    # Network tools
    xh
    wget
    whois
    openssl

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
    glab
    git-filter-repo
    du-dust
    ripgrep
    ripgrep-all
    tokei
    zip
    unzip
    mergiraf
    uutils-diffutils
    delta
    #diffoscope

    # Container tools
    distrobox
    bubblewrap
    appimage-run

    # System dev
    #lldb
    gdb
    cling # C++ repl
    evcxr # Rust repl
    lurk
    tracexec
    llvmPackages.bintools
    #darling

    # Nix dev
    alejandra
    nil
    nixd
    nix-tree
    statix
    manix
    devenv
    nix-prefetch-git
    nix-fast-build
    nix-init

    # Media tools
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    imagemagick
    oxipng
    gimp3

    # Mojo
    magic
    mojo

    # XR Desktop
    #monado
    #stardustxr
    #flatland
    #weston
  ];
}
