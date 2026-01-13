{
  programs.starship = {
    enable = true;
    settings = {
      command_timeout = 10000;
      time = {
        disabled = false;
        format = " [$time]($style) ";
      };
      status = {
        disabled = false;
      };
      directory = {
        truncation_length = 8;
        truncation_symbol = ".../";
        truncate_to_repo = false;
      };
    };
  };
}
