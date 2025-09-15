{pkgs, ...}: {
  systemd.services.dmcryptd = {
    description = "DMcrypt daemon";
    wantedBy = ["multi-user.target"];
    path = with pkgs; [util-linux cryptsetup];
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${./dmcryptd.py}";
      # You may also want to add these common settings:
      Restart = "on-failure";
      User = "root"; # or specify a different user
    };
  };

  services.udev = {
    packages = [
      pkgs.nrf-udev
      pkgs.segger-jlink
    ];

    extraRules = ''
      # FTDI
      SUBSYSTEM=="usb", ATTR{idVendor}=="0403", ATTR{idProduct}=="6011", MODE="0666"

      # Jetson
      SUBSYSTEM=="usb", ATTR{idVendor}=="0955", ATTR{idProduct}="7c18", MODE="0666"
    '';
  };
}
