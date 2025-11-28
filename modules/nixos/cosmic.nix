{pkgs, ...}: {
  environment = {
    systemPackages = with pkgs; [
      #cosmic-ext-applet-emoji-selector
      #cosmic-ext-applet-external-monitor-brightness
      cosmic-ext-calculator
      examine
      forecast
      tasks
      cosmic-ext-tweaks
      cosmic-player
      #cosmic-reader
      #stellarshot
    ];
    sessionVariables = {
      COSMIC_DATA_CONTROL_ENABLED = 1;
    };
  };
  services = {
    desktopManager.cosmic.enable = true;
    displayManager.cosmic-greeter.enable = true;
    system76-scheduler.enable = true;
  };
  # Fix Zed open urls: https://github.com/NixOS/nixpkgs/issues/189851#issuecomment-1759954096
  systemd.user.extraConfig = ''
    DefaultEnvironment="PATH=/run/wrappers/bin:/etc/profiles/per-user/%u/bin:/nix/var/nix/profiles/default/bin:/run/current-system/sw/bin"
  '';
}
