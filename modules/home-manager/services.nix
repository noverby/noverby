_: {
  services = {
    ssh-agent = {
      enable = true;
      # Cosmic has hardcoded socket path
      # https://github.com/pop-os/cosmic-session/blob/379ce30715f637075879feda784edc89231792cf/data/start-cosmic#L58
      socket = "keyring/ssh";
    };
    espanso = {
      enable = true;
      matches = {
        base = {
          matches = [
            {
              trigger = ":100";
              replace = "💯";
            }
          ];
        };
      };
    };
  };
}
