{inputs, ...}: {
  imports = with inputs.self.homeModules; [
    inputs.zen-browser.homeModules.default
    home
    systemd
    packages
    xdg
    file
    programs
  ];
}
