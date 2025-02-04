{
  pkgs,
  homeDirectory,
  ...
}: {
  systemd.user = {
    startServices = "sd-switch";
    services = {
      # xreal-air-driver = {
      # Unit = {
      # Description = "XREAL Air user-space driver";
      # After = "network.target";
      # };
      # Service = {
      # Type = "simple";
      # Environment = "HOME=${homeDirectory}";
      # ExecStart = "${pkgs.bash}/bin/bash -c '${pkgs.xr-linux-driver}/bin/xrealAirLinuxDriver'";
      # Restart = "on-failure";
      # };
      # Install = {
      # WantedBy = ["multi-user.target"];
      # };
      # };
    };
  };
}
