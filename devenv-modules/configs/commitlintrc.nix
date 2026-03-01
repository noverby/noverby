# Generate commitlintrc.yml with scope options dynamically derived from
# the top-level directories in the Nix flake source tree.
{
  pkgs,
  lib,
}: let
  flakeSrc = ../..;
  files = ["git" "readme" "license"];
  entries = builtins.readDir flakeSrc;
  ignoreDirs = ["target"];
  allDirs = builtins.attrNames (lib.filterAttrs (_: type: type == "directory") entries);
  dirs = builtins.sort (a: b: a < b) (
    map (name:
      if lib.hasPrefix "." name
      then lib.removePrefix "." name
      else name)
    (builtins.filter (name:
      !builtins.elem name ignoreDirs
      && !(lib.hasPrefix "." name && !builtins.elem (lib.removePrefix "." name) ["tangled"]))
    allDirs)
  );
  scopeOptionsYaml = lib.concatMapStringsSep "\n" (s: "      - ${s}") (dirs ++ files);
  commitlintrcContent =
    builtins.replaceStrings
    ["@SCOPE_OPTIONS@"]
    [scopeOptionsYaml]
    (builtins.readFile ./commitlintrc.yml);
in
  pkgs.writeText "commitlintrc.yml" commitlintrcContent
