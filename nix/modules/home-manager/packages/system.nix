{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    killall
    uutils-coreutils-noprefix
    xkill
    lsof
    wl-clipboard
    skim
    #waypipe
    wl-color-picker
    cryptsetup
  ];
}
