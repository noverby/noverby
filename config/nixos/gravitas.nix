{
  inputs,
  src,
  lib,
  ...
}: {
  system = "x86_64-linux";

  specialArgs = {
    inherit src inputs lib;
    stateVersion = "24.05";
  };

  modules = with inputs.self.nixosModules; [
    inputs.nixos-hardware
    .nixosModules
    .lenovo-thinkpad-p14s-amd-gen5
    inputs.catppuccin.nixosModules.catppuccin
    inputs.home-manager.nixosModules.home-manager
    inputs.ragenix.nixosModules.default
    inputs.self.hardware.thinkpad-t14-ryzen-7-pro
    nitrokey
    age
    cosmic
    gnome
    core
    programs
    services
    catppuccin
    home-manager
    xr
    veo
    cloud-hypervisor
  ];
}
