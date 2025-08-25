{
  inputs,
  src,
  ...
}: {
  specialArgs = {
    inherit src inputs;
    stateVersion = "25.05";
  };
  modules = with inputs; [
    home-manager.nixosModules.home-manager
    self.nixosModules.dell-xps-9320
    self.nixosModules.cosmic
    self.nixosModules.gnome
    self.nixosModules.base
    self.nixosModules.home-manager
  ];
}
