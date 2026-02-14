{pkgs, ...}: {
  programs.starship = {
    enable = true;
    settings = {
      format = "$all\${custom.jj}";
      custom.jj = {
        command = "prompt";
        format = "$output";
        ignore_timeout = true;
        shell = [pkgs.starship-jj "--ignore-working-copy" "starship"];
        use_stdin = false;
        "when" = true;
      };
    };
  };
}
