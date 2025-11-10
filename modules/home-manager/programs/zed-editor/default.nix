{
  config,
  lib,
  pkgs,
  ...
}: {
  programs.zed-editor = {
    enable = true;
    extensions = [
      "biome"
      "nix"
      "nickel"
      "typos"
      "nu"
      "just"
      "just-ls"
      "cargo-appraiser"
      "cargo-tom"
      "catppuccin-blur"
      "harper"
      "jj-lsp"
      "meson"
    ];
  };
  home = {
    activation = let
      configDir = "${config.xdg.configHome}/zed";
      settingsPath = "${configDir}/settings.json";
      keymapPath = "${configDir}/keymap.json";
      tasksPath = "${configDir}/tasks.json";

      userKeymaps = builtins.readFile ./keymap.json;
      userSettings = builtins.readFile ./settings.json;
      userTasks = builtins.readFile ./tasks.json;
    in {
      removeExistingZedSettings = lib.hm.dag.entryBefore ["checkLinkTargets"] ''
        rm -rf "${settingsPath}" "${keymapPath}"
      '';

      overwriteZedSymlink = lib.hm.dag.entryAfter ["linkGeneration"] ''
        mkdir -p "${configDir}"
        cat ${pkgs.writeText "zed-settings" userSettings} > "${settingsPath}"
        cat ${pkgs.writeText "zed-keymaps" userKeymaps} > "${keymapPath}"
        cat ${pkgs.writeText "zed-tasks" userTasks} > "${tasksPath}"
      '';
    };
  };
}
