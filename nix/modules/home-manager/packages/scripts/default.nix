{
  pkgs,
  lib,
  ...
}: {
  home = {
    packages = [
      (
        pkgs.writeScriptBin "vi" (lib.readFile ./vi)
      )
      (
        pkgs.writeScriptBin "uf" (lib.readFile ./uf)
      )
      (
        pkgs.writeScriptBin "zed-uf" (lib.readFile ./zed-uf)
      )
      (
        pkgs.writeScriptBin "zellij-cwd" (lib.readFile ./zellij-cwd)
      )
      (
        pkgs.writeScriptBin "nix-flamegraph" (lib.readFile ./nix-flamegraph)
      )
    ];
  };
}
