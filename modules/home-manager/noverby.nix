{inputs, ...}: {
  imports = with inputs.self.homeModules; [
    inputs.zen-browser.homeModules.default
    inputs.spicetify-nix.homeManagerModules.spicetify
    inputs.catppuccin.homeModules.catppuccin
    home
    systemd
    packages
    xdg
    file
    programs
    catppuccin
  ];
}
