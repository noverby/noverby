{pkgs, ...}: {
  home.packages = with pkgs; [
    # Pop
    pop-launcher
    pop-icon-theme
    pop-gtk-theme

    # General apps
    genact
    #bitwarden
    zen-browser
    slack
    fragments
    evince
    spotify
    protonmail-desktop
    bitwarden-desktop
    mpv
    onlyoffice-desktopeditors
    gnome-tweaks
    dconf-editor
    celeste
    gnome-network-displays
    gnome-system-monitor
    file-roller
    eyedropper
    wireplumber
    gnome-disk-utility

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
    tailspin

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

    # File tools
    helix
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
    diffoscope

    # Container tools
    distrobox
    bubblewrap
    appimage-run

    # System dev
    lldb
    gdb
    cling # C++ repl
    evcxr # Rust repl
    lurk
    tracexec
    #darling

    # Nix dev
    alejandra
    nil
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

    # Mojo
    magic

    # XR Desktop
    #monado
    #stardustxr
    #flatland
    #weston
  ];
}
