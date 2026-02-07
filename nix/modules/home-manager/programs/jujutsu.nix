{
  programs.jujutsu = {
    enable = true;
    settings = {
      user = {
        name = "Niclas Overby";
        email = "niclas@overby.me";
      };
      ui = {
        pager = "delta";
        diff-formatter = ":git";
      };
      merge-tools.mergiraf = {
        program = "mergiraf";
        merge-args = ["merge" "$base" "$left" "$right" "-o" "$output" "--fast"];
        merge-conflict-exit-codes = [1];
        conflict-marker-style = "git";
      };
    };
  };
}
