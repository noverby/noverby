{inputs, ...}: {
  home = {
    username = "noverby";
    homeDirectory = "/home/noverby";
  };
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
    services
    catppuccin
    vibe
    xr
  ];
}
