{
  config,
  homeDirectory,
  pkgs,
  ...
}:
with config.lib.file; {
  home.file = {
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
    ".local/bin/vi" = {
      executable = true;
      source = ./bin/vi;
    };
    ".local/bin/uf" = {
      executable = true;
      source = ./bin/uf;
    };
    ".local/bin/zed-uf" = {
      executable = true;
      source = ./bin/zed-uf;
    };
    ".local/bin/zellij-cwd" = {
      executable = true;
      source = ./bin/zellij-cwd;
    };
    ".npmrc".source = ./config/npmrc.ini;
    ".pystartup".source = ./config/pystartup;
    ".config/pop-shell/config.json".source = ./config/pop-shell/config.json;
    ".config/mpv/mpv.conf".source = ./config/mpv/mpv.conf;
    ".local/share/wallpapers/current.png".source = "${(pkgs.nix-wallpaper.override {
      preset = "catppuccin-mocha";
      logoSize = 10;
    })}/share/wallpapers/nixos-wallpaper.png";
  };
}
