{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    helix
    file
    unixtools.xxd
    fd
    tre
    hexyl
    git-filter-repo
    dust
    ripgrep
    ripgrep-all
    tokei
    zip
    unzip
    p7zip
    uutils-diffutils
    ast-grep
    diffoscope
    jless
    television
    yq-go # Needed by prettybat
  ];
}
