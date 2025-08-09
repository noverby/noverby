{inputs, ...}: {
  imports = with inputs.self.homeModules; [
    inputs.zen-browser.homeModules.default
    inputs.spicetify-nix.homeManagerModules.spicetify
    home
    systemd
    packages
    xdg
    file
    programs
  ];
}
