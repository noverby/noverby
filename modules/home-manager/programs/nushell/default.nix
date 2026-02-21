{pkgs, ...}: {
  programs.nushell = {
    enable = true;
    configFile.source = ./config.nu;
    envFile.text = ''
      $env.SHELL = "${pkgs.nushell}/bin/nu"
    '';
  };

  programs.nu-plugin-tramp.enable = true;
}
