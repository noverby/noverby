{pkgs, ...}: {
  services = {
    desktopManager.gnome.enable = true;
    gnome = {
      gnome-browser-connector.enable = true;
    };
  };
  environment.gnome.excludePackages = with pkgs; [epiphany];
  # Unmanaged gnome-extensions deps
  #environment.sessionVariables = with pkgs; {
  #  GI_TYPELIB_PATH = map (pkg: "${pkg}/lib/girepository-1.0") [vte pango harfbuzz gtk3 gdk-pixbuf at-spi2-core];
  #};
}
