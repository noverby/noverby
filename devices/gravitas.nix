{
  inputs,
  src,
  ...
}: {
  specialArgs = {
    inherit src inputs;
    stateVersion = "24.05";
  };
  modules = with inputs; [
    nixos-hardware
    .nixosModules
    .lenovo-thinkpad-p14s-amd-gen5
    catppuccin.nixosModules.catppuccin
    home-manager.nixosModules.home-manager
    self.nixosModules.thinkpad-t14-ryzen-7-pro
    self.nixosModules.cosmic
    self.nixosModules.gnome
    self.nixosModules.base
    self.nixosModules.home-manager
    self.nixosModules.veo
  ];
}
