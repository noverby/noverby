{
  username,
  lib,
  ...
}: {
  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;
    matchBlocks = {
      "*" = {
        addKeysToAgent = "yes";
        controlMaster = "auto";
        controlPath = "~/.ssh/socket/%r@%h:%p";
        controlPersist = "120";
        forwardAgent = true;
      };
      localhost = {
        hostname = "localhost";
        user = username;
      };
    };
  };
  # Needed for ssh to work in buildFHSEnv
  home.activation = {
    copySSHConfig = lib.hm.dag.entryAfter ["linkGeneration"] ''
      configPath="$HOME/.ssh/config"
      if [ -L "$configPath" ]; then
        target="$(readlink "$configPath")"
        run rm "$configPath"
        run cp "$target" "$configPath"
        run chmod 600 "$configPath"
      fi
    '';
  };
}
