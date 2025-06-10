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
    sockets.gcr-ssh-agent = {
      Unit = {
        Description = "GCR SSH Agent Socket";
      };
      Socket = {
        ListenStream = "%t/gcr/ssh";
        DirectoryMode = "0700";
      };
      Install = {
        WantedBy = ["sockets.target"];
      };
    };

    services.gcr-ssh-agent = {
      Unit = {
        Description = "GCR SSH Agent";
        Requires = ["gcr-ssh-agent.socket"];
        After = ["gcr-ssh-agent.socket"];
      };
      Service = {
        Type = "simple";
        ExecStart = "${pkgs.gcr_4}/libexec/gcr-ssh-agent --base-dir %t/gcr";
        StandardError = "journal";
      };
    };
  };
}
