{...}: {
  programs.git = {
    enable = true;
    userName = "Niclas Overby";
    userEmail = "niclas@overby.me";
    lfs = {
      enable = true;
    };
    difftastic = {
      enable = true;
    };
    extraConfig = {
      core = {
        autocrlf = "input";
        editor = "vi --wait";
      };
      init = {
        defaultBranch = "main";
      };
      push = {
        default = "simple";
        autoSetupRemote = true;
      };
      pull = {
        rebase = true;
      };
      merge = {
        tool = "vi";
      };
      mergetool = {
        vi = {
          cmd = "vi --wait $MERGED";
        };
      };
      diff = {
        tool = "vi";
      };
      difftool = {
        vi = {
          cmd = "vi --wait --diff $LOCAL $REMOTE";
        };
      };
      color = {
        ui = "auto";
      };
      credential = {
        helper = "store";
      };
      filter.lfs = {
        clean = "git-lfs clean -- %f";
        smudge = "git-lfs smudge -- %f";
        process = "git-lfs filter-process";
        required = true;
      };
    };
  };
}
