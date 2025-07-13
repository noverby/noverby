{
  config,
  lib,
  pkgs,
  ...
}: let
  configDir = "${config.xdg.configHome}/zed";
  settingsPath = "${configDir}/settings.json";
  keymapPath = "${configDir}/keymap.json";

  userKeymaps = builtins.fromJSON (builtins.readFile ./zed-editor/keymap.json);
  userSettings = builtins.fromJSON (builtins.readFile ./zed-editor/settings.json);
in {
  programs.zed-editor = {
    enable = true;
    extensions = [
      "biome"
      "nix"
      "nu"
      "zed-just"
      "zed-just-ls"
    ];
  };
  home = {
    activation = {
      removeExistingZedSettings = lib.hm.dag.entryBefore ["checkLinkTargets"] ''
        rm -rf "${settingsPath}" "${keymapPath}"
      '';

      overwriteZedSymlink = let
        jsonSettings = pkgs.writeText "tmp_zed_settings" (builtins.toJSON userSettings);
        jsonKeymaps = pkgs.writeText "tmp_zed_keymaps" (builtins.toJSON userKeymaps);
      in
        lib.hm.dag.entryAfter ["linkGeneration"] ''
          mkdir -p "${configDir}"
          rm -rf "${settingsPath}" "${keymapPath}"
          cat ${jsonSettings} | ${pkgs.jq}/bin/jq --monochrome-output > "${settingsPath}"
          cat ${jsonKeymaps} | ${pkgs.jq}/bin/jq --monochrome-output > "${keymapPath}"
          chmod u+w "${settingsPath}" "${keymapPath}"
        '';
    };
  };
}
