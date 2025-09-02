{pkgs, ...}: {
  systemd.services.dmcrypt-daemon = {
    description = "DMcrypt daemon";
    wantedBy = ["multi-user.target"];
    path = with pkgs; [util-linux];
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${./dmcryptd.py}";
      # You may also want to add these common settings:
      Restart = "on-failure";
      User = "root"; # or specify a different user
    };
  };
}
