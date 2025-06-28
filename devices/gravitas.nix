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
    .framework-13th-gen-intel
    home-manager.nixosModules.home-manager
    self.nixosModules.framework-13th-gen-intel
    self.nixosModules.cosmic
    self.nixosModules.gnome
    self.nixosModules.configuration
    self.nixosModules.home-manager
  ];
}
