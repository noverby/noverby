{
  inputs,
  src,
  lib,
  ...
}: {
  system = "x86_64-linux";

  specialArgs = {
    inherit src inputs lib;
    stateVersion = "25.05";
  };

  modules = with inputs.self.nixosModules; [
    inputs.home-manager.nixosModules.home-manager
    inputs.self.hardware.dell-xps-9320
    inputs.self.desktops.cosmic
    inputs.self.desktops.gnome
    core
    home-manager
  ];
}
