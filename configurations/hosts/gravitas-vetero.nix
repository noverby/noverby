{
  inputs,
  src,
  ...
}: {
  system = "x86_64-linux";

  specialArgs = {
    inherit src inputs;
    stateVersion = "25.05";
  };

  modules = with inputs.self.nixosModules; [
    inputs.home-manager.nixosModules.home-manager
    dell-xps-9320
    cosmic
    gnome
    core
    home-manager
  ];
}
