{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    imagemagick
    oxipng
    gimp3
  ];
}
