{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
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
  ];
}
