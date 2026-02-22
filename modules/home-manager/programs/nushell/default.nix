{pkgs, ...}: {
  programs.nushell = {
    enable = true;
    package = pkgs.pkgsUnstable.nushell;
    configFile.source = ./config.nu;
    envFile.text = ''
      $env.SHELL = "${pkgs.nushell}/bin/nu"
    '';
  };

  programs.nu-plugin-tramp.enable = true;
}
