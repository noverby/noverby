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
        pkgs.writeShellScriptBin "vi" ./bin/vi
      )
      (
        pkgs.writeShellScriptBin "uf" ./bin/uf
      )
      (
        pkgs.writeShellScriptBin "zed-uf" ./bin/zed-uf
      )
      (
        pkgs.writeShellScriptBin "zellij-cwd" ./bin/zellij-cwd
      )
      (
        pkgs.writeShellScriptBin "nix-flamegraph" ./bin/nix-flamegraph
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
      ".npmrc".source = ./config/npmrc.ini;
      ".config/pop-shell/config.json".source = ./config/pop-shell/config.json;
      ".config/mpv/mpv.conf".source = ./config/mpv/mpv.conf;
      ".local/share/wallpapers/current.png".source = "${(pkgs.nix-wallpaper.override {
        preset = "catppuccin-mocha";
        logoSize = 10;
      })}/share/wallpapers/nixos-wallpaper.png";
    };
  };
}
