{
  pkgs,
  username,
  homeDirectory,
  stateVersion,
  ...
}: {
  home = {
    inherit username homeDirectory stateVersion;
    enableDebugInfo = true;
    shell = {
      enableBashIntegration = true;
      enableNushellIntegration = true;
    };
    shellAliases = {
      open = "xdg-open";
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
      DIRENV_LOG_FORMAT = "";
      PYTHONSTARTUP = "${homeDirectory}/.pystartup";
      GRANTED_ALIAS_CONFIGURED = "true";

      # GStreamer
      GST_PLUGIN_SYSTEM_PATH_1_0 = with pkgs.gst_all_1; "${gstreamer.out}/lib/gstreamer-1.0:${gst-plugins-base}/lib/gstreamer-1.0:${gst-plugins-good}/lib/gstreamer-1.0";

      # XR
      XR_RUNTIME_JSON = "${pkgs.monado}/share/openxr/1/openxr_monado.json";
      XRT_COMPOSITOR_FORCE_XCB = "1";
      XRT_COMPOSITOR_XCB_FULLSCREEN = "1";

      DEVENV_ENABLE_HOOKS = "1";
    };
  };
}
