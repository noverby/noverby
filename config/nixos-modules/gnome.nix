{pkgs, ...}: {
  services = {
    gnome = {
      gnome-keyring.enable = true;
      gnome-browser-connector.enable = true;
    };
    xserver = {
      displayManager.gdm = {
        enable = true;
        wayland = true;
      };
      desktopManager.gnome.enable = true;
    };
  };
  environment.gnome.excludePackages = with pkgs; [epiphany];
  # Unmanaged gnome-extensions deps
  environment.sessionVariables = with pkgs; {
    GI_TYPELIB_PATH = map (pkg: "${pkg}/lib/girepository-1.0") [vte pango harfbuzz gtk3 gdk-pixbuf at-spi2-core];
  };
}
