{
  stateVersion,
  pkgs,
  config,
  ...
}: {
  home = {
    inherit stateVersion;
    enableDebugInfo = true;
    shell = {
      enableBashIntegration = true;
      enableNushellIntegration = true;
    };
    shellAliases = {
      xopen = "xdg-open";
      diff = "batdiff";
      ga = "git add";
      gc = "git commit";
      gcm = "git commit -m";
      gca = "git commit --amend";
      gcn = "git commit --no-verify";
      gcp = "git cherry-pick";
      gd = "git diff";
      gf = "git fetch";
      gl = "git log --oneline --no-abbrev-commit";
      glg = "git log --graph";
      gpl = "git pull";
      gps = "git push";
      gpf = "git push -f";
      gr = "git rebase";
      gri = "git rebase -i";
      grc = "git rebase --continue";
      gm = "git merge";
      gs = "git status";
      gsh = "git stash";
      gsha = "git stash apply";
      gsw = "git switch";
      gundo = "git reset HEAD~1 --soft";
      gbm = "gh pr comment --body 'bors merge'";
      gbc = "gh pr comment --body 'bors cancel'";
      gpc = "gh pr create --draft --fill";
      gpv = "gh pr view --web";
      du = "dust";
      cat = "prettybat";
      find = "fd";
      grep = "rg";
      man = "tldr";
      top = "btm";
      cd = "z";
      bg = "pueue";
      ping = "gping";
      time = "hyperfine";
      tree = "tre";
      zed = "zeditor";
      optpng = "oxipng";
      firefox-dev = "firefox -start-debugger-server 6000 -P dev http://localhost:3000";
      zen-dev = "zen -start-debugger-server 6000 -P dev http://localhost:3000";
    };
    sessionVariables = {
      EDITOR = "vi";
      VISUAL = "vi";
      BATDIFF_USE_DELTA = "true";
      PYTHON_HISTORY = "~/.local/share/python/history";
      GRANTED_ALIAS_CONFIGURED = "true";
    };
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
