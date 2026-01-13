{
  config,
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
      Pictures.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Pictures";
      Documents.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Documents";
      Desktop.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Desktop";
      Videos.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Videos";
      Music.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Music";
      Templates.source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Templates";
      "Work/proj".source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Projects";
      "Work/wiki".source = mkOutOfStoreSymlink "${config.home.homeDirectory}/Sync/Documents/Wiki";
      "Work/tmp/.keep".source = builtins.toFile "keep" "";
      ".ssh/socket/.keep".source = builtins.toFile "keep" "";
      ".local/share/wallpapers/current.png".source = "${(pkgs.nix-wallpaper.override {
        preset = "catppuccin-mocha";
        logoSize = 10;
      })}/share/wallpapers/nixos-wallpaper.png";
      ".config/helix/config.toml".text = ''
        # System  clipboard
        p = "paste_clipboard_after"
        P = "paste_clipboard_before"
        y = "yank_to_clipboard"
        Y = "yank_joined_to_clipboard"
        R = "replace_selections_with_clipboard"
        d = ["yank_to_clipboard", "delete_selection_noyank"]
      '';
    };
  };
}
