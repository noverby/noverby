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
    hasSecrets = true;
  };

  modules = with inputs.self.nixosModules; [
    inputs.catppuccin.nixosModules.catppuccin
    inputs.home-manager.nixosModules.home-manager
    inputs.self.hardware.dell-xps-9320
    inputs.self.desktops.cosmic
    inputs.self.desktops.gnome
    inputs.ragenix.nixosModules.default
    age
    core
    programs
    services
    catppuccin
    home-manager
    cloud-hypervisor
    tangled-spindle-nix
    {
      age.secrets.spindle-token = {
        file = inputs.self.secrets.spindle-token;
        mode = "600";
      };

      services.tangled-spindles.default = {
        enable = true;
        hostname = "spindle.overby.me";
        owner = "did:plc:eukcx4amfqmhfrnkix7zwm34";
        tokenFile = "/run/agenix/spindle-token";
        engine = {
          maxJobs = 2;
          queueSize = 100;
          workflowTimeout = "2h";
        };
      };

      services.caddy = {
        enable = true;
        virtualHosts."spindle.overby.me" = {
          extraConfig = ''
            reverse_proxy localhost:6555
          '';
        };
      };

      security.sudo.wheelNeedsPassword = false;
      networking = {
        hostName = "home";
        networkmanager.unmanaged = ["enp0s13f0u3u4"];
        interfaces.enp0s13f0u3u4 = {
          useDHCP = false;
          ipv4.addresses = [
            {
              address = "10.0.0.2";
              prefixLength = 24;
            }
          ];
        };
        defaultGateway = "10.0.0.1";
        nameservers = ["194.242.2.2"];
        firewall.allowedTCPPorts = [22 80 443];
      };
    }
  ];
}
