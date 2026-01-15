{pkgs, ...}: {
  home = {
    packages = [
      (
        pkgs.writeScriptBin "vi" (builtins.readFile ./vi)
      )
      (
        pkgs.writeScriptBin "uf" (builtins.readFile ./uf)
      )
      (
        pkgs.writeScriptBin "zed-uf" (builtins.readFile ./zed-uf)
      )
      (
        pkgs.writeScriptBin "zellij-cwd" (builtins.readFile ./zellij-cwd)
      )
      (
        pkgs.writeScriptBin "nix-flamegraph" (builtins.readFile ./nix-flamegraph)
      )
    ];
  };
}
