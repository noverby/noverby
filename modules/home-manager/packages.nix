{pkgs, ...}: {
  home.packages = with pkgs; [
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
    pavucontrol

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
    wl-color-picker

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
    gping
    bandwhich
    hexyl
    hyperfine
    jless
    rustscan
    tre

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
    p7zip
    mergiraf
    uutils-diffutils
    delta
    ast-grep
    diffoscope
    simg2img
    cryptsetup
    binwalk

    # Container tools
    distrobox
    bubblewrap
    appimage-run
    cloud-hypervisor

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
    envy
    nix-tree
    manix
    devenv
    nix-prefetch-git
    nix-fast-build
    nix-init
    comma
    nurl

    # Media tools
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    imagemagick
    oxipng
    gimp3
  ];
}
