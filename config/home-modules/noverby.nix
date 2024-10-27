{inputs, ...}: {
  imports = with inputs.self.homeModules; [home systemd packages gnome xdg file programs];
}
