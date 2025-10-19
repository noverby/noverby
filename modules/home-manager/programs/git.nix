_: {
  programs.git = {
    enable = true;
    lfs = {
      enable = true;
    };
    settings = {
      user = {
        name = "Niclas Overby";
        email = "niclas@overby.me";
      };
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
        mergiraf = {
          name = "mergiraf";
          driver = "mergiraf merge --git %O %A %B -s %S -x %X -y %Y -p %P -l %L";
        };
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
    attributes = [
      "*.java merge=mergiraf"
      "*.kt merge=mergiraf"
      "*.rs merge=mergiraf"
      "*.go merge=mergiraf"
      "*.js merge=mergiraf"
      "*.jsx merge=mergiraf"
      "*.mjs merge=mergiraf"
      "*.json merge=mergiraf"
      "*.yml merge=mergiraf"
      "*.yaml merge=mergiraf"
      "*.toml merge=mergiraf"
      "*.html merge=mergiraf"
      "*.htm merge=mergiraf"
      "*.xhtml merge=mergiraf"
      "*.xml merge=mergiraf"
      "*.c merge=mergiraf"
      "*.h merge=mergiraf"
      "*.cc merge=mergiraf"
      "*.cpp merge=mergiraf"
      "*.hpp merge=mergiraf"
      "*.cs merge=mergiraf"
      "*.dart merge=mergiraf"
      "*.dts merge=mergiraf"
      "*.scala merge=mergiraf"
      "*.sbt merge=mergiraf"
      "*.ts merge=mergiraf"
      "*.tsx merge=mergiraf"
      "*.py merge=mergiraf"
      "*.php merge=mergiraf"
      "*.phtml merge=mergiraf"
      "*.sol merge=mergiraf"
      "*.lua merge=mergiraf"
      "*.rb merge=mergiraf"
    ];
  };
}
