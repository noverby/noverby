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
    inputs.ragenix.nixosModules.default
    inputs.self.hardware.dell-xps-9320
    inputs.self.desktops.cosmic
    inputs.self.desktops.gnome
    age
    core
    programs
    services
    catppuccin
    home-manager
    cloud-hypervisor
    tangled-spindle
    wiki-auth
    ({config, ...}: {
      age.secrets."wiki-auth-env" = {
        file = inputs.self.secrets.wiki-auth-env;
        path = "/run/agenix/wiki-auth-env";
        owner = "root";
        group = "root";
        mode = "600";
      };
      services.wiki-auth = {
        enable = true;
        nhostSubdomain = "pgvhpsenoifywhuxnybq";
        nhostRegion = "eu-central-1";
        hasuraEndpoint = "https://pgvhpsenoifywhuxnybq.hasura.eu-central-1.nhost.run/v1/graphql";
        publicUrl = "https://auth.radikal.wiki";
        smtpHost = "smtp.zoho.com";
        smtpPort = 465;
        smtpFrom = "noreply@radikal.wiki";
        smtpSecure = true;
        serverDir = "${src}/wiki/server";
        environmentFile = config.age.secrets."wiki-auth-env".path;
      };
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
    })
  ];
}
