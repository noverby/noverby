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
  entries = lib.readDir src;
  ignoreDirs = ["target" "result" "result-man"];
  visibleDirs = lib.filter (name:
    !lib.elem name ignoreDirs
    && !(lib.hasPrefix "." name && !lib.elem (lib.removePrefix "." name) ["tangled"]))
  (lib.attrNames (lib.filterAttrs (_: type: type == "directory") entries));

  # Normalize dotfile dirs (e.g. .tangled -> tangled)
  normalizeName = name:
    if lib.hasPrefix "." name
    then lib.removePrefix "." name
    else name;

  # For each top-level dir, discover immediate subdirectories as "parent/child" scopes.
  subScopes = dir: let
    dirPath = src + "/${dir}";
    subEntries = lib.readDir dirPath;
    subDirs =
      lib.filter (name: !lib.hasPrefix "." name)
      (lib.attrNames (lib.filterAttrs (_: type: type == "directory") subEntries));
  in
    map (sub: "${normalizeName dir}/${sub}") subDirs;

  topLevel = map normalizeName visibleDirs;
  nested = lib.concatMap subScopes visibleDirs;
  allScopes = lib.sort (a: b: a < b) (topLevel ++ nested ++ files);
  scopeOptionsYaml = lib.concatMapStringsSep "\n" (s: "      - ${s}") allScopes;
  commitlintrcContent =
    lib.replaceStrings
    ["@SCOPE_OPTIONS@"]
    [scopeOptionsYaml]
    (lib.readFile ./commitlintrc.yml);
in
  pkgs.writeText "commitlintrc.yml" commitlintrcContent
