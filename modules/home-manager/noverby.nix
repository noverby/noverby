{inputs, ...}: {
  imports = with inputs.self.homeModules; [home systemd packages xdg file programs];
}
