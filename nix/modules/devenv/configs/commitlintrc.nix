# Generate commitlintrc.yml with scope options dynamically derived from
# directories in the Nix flake source tree.
# Discovers both top-level dirs (e.g. "nix", "slides") and their immediate
# subdirectories (e.g. "rust/nixos", "nix/lib", "web/wiki").
{
  pkgs,
  lib,
  src,
}: let
  files = ["git" "readme" "license"];
  entries = builtins.readDir src;
  ignoreDirs = ["target" "result" "result-man"];
  visibleDirs = builtins.filter (name:
    !builtins.elem name ignoreDirs
    && !(lib.hasPrefix "." name && !builtins.elem (lib.removePrefix "." name) ["tangled"]))
  (builtins.attrNames (lib.filterAttrs (_: type: type == "directory") entries));

  # Normalize dotfile dirs (e.g. .tangled -> tangled)
  normalizeName = name:
    if lib.hasPrefix "." name
    then lib.removePrefix "." name
    else name;

  # For each top-level dir, discover immediate subdirectories as "parent/child" scopes.
  subScopes = dir: let
    dirPath = src + "/${dir}";
    subEntries = builtins.readDir dirPath;
    subDirs =
      builtins.filter (name: !lib.hasPrefix "." name)
      (builtins.attrNames (lib.filterAttrs (_: type: type == "directory") subEntries));
  in
    map (sub: "${normalizeName dir}/${sub}") subDirs;

  topLevel = map normalizeName visibleDirs;
  nested = lib.concatMap subScopes visibleDirs;
  allScopes = builtins.sort (a: b: a < b) (topLevel ++ nested ++ files);
  scopeOptionsYaml = lib.concatMapStringsSep "\n" (s: "      - ${s}") allScopes;
  commitlintrcContent =
    builtins.replaceStrings
    ["@SCOPE_OPTIONS@"]
    [scopeOptionsYaml]
    (builtins.readFile ./commitlintrc.yml);
in
  pkgs.writeText "commitlintrc.yml" commitlintrcContent
