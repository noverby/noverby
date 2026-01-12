{pkgs, ...}: {
  home = {
    sessionPath = [
      "${pkgs.pinentry-gnome3}/bin"
    ];
    packages = with pkgs.pkgsUnstable; [
      ragenix
      rage
      keyutils
      (
        pkgs.writeScriptBin "pinentry" ''
          #!${pkgs.stdenv.shell}
          pinentry-linux-sessioncache ${pkgs.pinentry-gnome3}/bin/pinentry-gnome3
        ''
      )
      (
        pkgs.writeScriptBin "pinentry-linux-sessioncache" (builtins.readFile ./pinentry-linux-sessioncache)
      )
    ];
  };
}
