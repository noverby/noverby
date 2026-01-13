{
  networking = {
    hostName = "gravitas";
    networkmanager = {
      enable = true;
      dns = "systemd-resolved";
    };
  };
}
