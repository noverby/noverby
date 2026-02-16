{pkgs, ...}: {
  systemd.package = pkgs.systemd-rs-systemd;
}
