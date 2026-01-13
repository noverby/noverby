{
  config,
  pkgs,
  ...
}: {
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
    file = let
      symlink = config.lib.file.mkOutOfStoreSymlink;
      inherit (config.home) homeDirectory;
    in {
      Pictures.source = symlink "${homeDirectory}/Sync/Pictures";
      Documents.source = symlink "${homeDirectory}/Sync/Documents";
      Desktop.source = symlink "${homeDirectory}/Sync/Desktop";
      Videos.source = symlink "${homeDirectory}/Sync/Videos";
      Music.source = symlink "${homeDirectory}/Sync/Music";
      Templates.source = symlink "${homeDirectory}/Sync/Templates";
      "Work/proj".source = symlink "${homeDirectory}/Sync/Projects";
      "Work/wiki".source = symlink "${homeDirectory}/Sync/Documents/Wiki";
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
