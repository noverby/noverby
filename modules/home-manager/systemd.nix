{
  ...
}: {
  systemd.user = {
    startServices = "sd-switch";
    services = {};
  };
}
