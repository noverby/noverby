{
  config,
  lib,
  pkgs,
  ...
}: let
  vscodePname = config.programs.vscode.package.pname;
  configDir =
    {
      "vscode" = "Code";
      "vscode-insiders" = "Code - Insiders";
      "vscodium" = "VSCodium";
    }
    .${
      vscodePname
    };
  settingsPath = "${config.xdg.configHome}/${configDir}/User/settings.json";
  keybindingsPath = "${config.xdg.configHome}/${configDir}/User/keybindings.json";
in {
  home = {
    activation = {
      removeExistingVSCodeSettings = lib.hm.dag.entryBefore ["checkLinkTargets"] ''
        rm -rf "${settingsPath}" "${keybindingsPath}"
      '';

      overwriteVSCodeSymlink = let
        inherit (config.programs.vscode.profiles.default) userSettings;
        jsonSettings = pkgs.writeText "tmp_vscode_settings" (builtins.toJSON userSettings);
        inherit (config.programs.vscode.profiles.default) keybindings;
        jsonKeybindings = pkgs.writeText "tmp_vscode_keybindings" (builtins.toJSON keybindings);
      in
        lib.hm.dag.entryAfter ["linkGeneration"] ''
          rm -rf "${settingsPath}" "${keybindingsPath}"
          cat ${jsonSettings} | ${pkgs.jq}/bin/jq --monochrome-output > "${settingsPath}"
          cat ${jsonKeybindings} | ${pkgs.jq}/bin/jq --monochrome-output > "${keybindingsPath}"
        '';
    };
  };

  programs.vscode = {
    enable = true;
    package = pkgs.vscodium;
    profiles.default = {
      extensions = with pkgs.vscode-extensions; [
        mkhl.direnv
        jnoortheen.nix-ide
        kamadorueda.alejandra
        rust-lang.rust-analyzer
        tamasfe.even-better-toml
        ms-python.python
        ms-vscode.hexeditor
        esbenp.prettier-vscode
        thenuprojectcontributors.vscode-nushell-lang
        ms-azuretools.vscode-docker
      ];
      userSettings = builtins.fromJSON (builtins.readFile ./settings.json);
      keybindings = builtins.fromJSON (builtins.readFile ./keybindings.json);
    };
  };
}
