{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    killall
    uutils-coreutils-noprefix
    xorg.xkill
    lsof
    wl-clipboard
    skim
    #waypipe
    wl-color-picker
    cryptsetup
  ];
}
