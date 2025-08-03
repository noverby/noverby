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
    home-manager.nixosModules.home-manager
    self.nixosModules.thinkpad-t14-ryzen-7-pro
    self.nixosModules.cosmic
    self.nixosModules.gnome
    self.nixosModules.configuration
    self.nixosModules.home-manager
  ];
}
