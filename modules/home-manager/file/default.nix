{
  config,
  homeDirectory,
  pkgs,
  ...
}:
with config.lib.file; {
  home = {
    packages = [
      (
        pkgs.writeScriptBin "vi" (builtins.readFile ./bin/vi)
      )
      (
        pkgs.writeScriptBin "uf" (builtins.readFile ./bin/uf)
      )
      (
        pkgs.writeScriptBin "zed-uf" (builtins.readFile ./bin/zed-uf)
      )
      (
        pkgs.writeScriptBin "zellij-cwd" (builtins.readFile ./bin/zellij-cwd)
      )
      (
        pkgs.writeScriptBin "nix-flamegraph" (builtins.readFile ./bin/nix-flamegraph)
      )
    ];
    file = {
      Pictures.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Pictures";
      Documents.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Documents";
      Desktop.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Desktop";
      Videos.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Videos";
      Music.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Music";
      Templates.source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Templates";
      "Work/proj".source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Projects";
      "Work/wiki".source = mkOutOfStoreSymlink "${homeDirectory}/Sync/Documents/Wiki";
      "Work/tmp/.keep".source = builtins.toFile "keep" "";
      ".ssh/socket/.keep".source = builtins.toFile "keep" "";
      ".local/share/wallpapers/current.png".source = "${(pkgs.nix-wallpaper.override {
        preset = "catppuccin-mocha";
        logoSize = 10;
      })}/share/wallpapers/nixos-wallpaper.png";
    };
  };
}
