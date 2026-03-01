{inputs, ...}: {
  imports = [inputs.tangled.nixosModules.spindle];

  services.tangled.spindle = {
    enable = true;
    server = {
      hostname = "spindle.overby.me";
      owner = "did:plc:eukcx4amfqmhfrnkix7zwm34";
      maxJobCount = 2;
      queueSize = 100;
    };
    pipelines = {
      nixery = "nixery.tangled.sh";
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

  networking.firewall.allowedTCPPorts = [80 443];
}
