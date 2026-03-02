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
    hasSecrets = false;
  };

  modules = with inputs.self.nixosModules; [
    inputs.catppuccin.nixosModules.catppuccin
    inputs.home-manager.nixosModules.home-manager
    inputs.self.hardware.dell-xps-9320
    inputs.self.desktops.cosmic
    inputs.self.desktops.gnome
    core
    programs
    services
    catppuccin
    home-manager
    cloud-hypervisor
    tangled-spindle
    wiki-auth
    {
      security.sudo.wheelNeedsPassword = false;
      networking = {
        hostName = "home";
        interfaces.enp0s13f0u3u4.ipv4.addresses = [
          {
            address = "10.0.0.2";
            prefixLength = 24;
          }
        ];
        defaultGateway = "10.0.0.1";
        nameservers = ["194.242.2.2"];
        firewall.allowedTCPPorts = [22];
      };
    }
  ];
}
