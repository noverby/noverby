{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    # Very serious tools
    genact
    fortune-kind
    microfetch
  ];
}
