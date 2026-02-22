# Directory auto-discovery for nix-workspace
#
# Scans convention directories (packages/, shells/, etc.) for .ncl files
# and maps them to output names.
#
# Convention:
#   packages/hello.ncl      → packages.hello
#   packages/default.ncl    → packages.<workspace-name> (in subworkspaces)
#   shells/default.ncl      → devShells.default
#
{lib}: let
  # Default convention directory mappings
  # Maps convention name → { dir, output }
  defaultConventions = {
    packages = {
      dir = "packages";
      output = "packages";
    };
    shells = {
      dir = "shells";
      output = "devShells";
    };
    modules = {
      dir = "modules";
      output = "nixosModules";
    };
    home = {
      dir = "home";
      output = "homeModules";
    };
    overlays = {
      dir = "overlays";
      output = "overlays";
    };
    machines = {
      dir = "machines";
      output = "nixosConfigurations";
    };
    templates = {
      dir = "templates";
      output = "templates";
    };
    checks = {
      dir = "checks";
      output = "checks";
    };
    lib = {
      dir = "lib";
      output = "lib";
    };
  };

  # Apply user convention overrides to the defaults.
  # Users can change directory paths or disable auto-discovery.
  applyConventionOverrides = conventions: overrides:
    lib.mapAttrs (
      name: conv:
        if builtins.hasAttr name overrides
        then let
          ovr = overrides.${name};
        in
          conv
          // (lib.optionalAttrs (ovr ? path) {dir = ovr.path;})
          // (lib.optionalAttrs (ovr ? auto-discover) {autoDiscover = ovr.auto-discover;})
        else conv // {autoDiscover = true;}
    )
    conventions;

  # List .ncl files in a directory and return a name → path mapping.
  #
  # Given workspaceRoot and a relative directory:
  #   packages/hello.ncl  →  { hello = /absolute/path/packages/hello.ncl; }
  #   packages/default.ncl → { default = /absolute/path/packages/default.ncl; }
  #
  discoverNclFiles = workspaceRoot: relativeDir: let
    dirPath = workspaceRoot + "/${relativeDir}";
  in
    if builtins.pathExists dirPath
    then let
      entries = builtins.readDir dirPath;
      nclEntries =
        lib.filterAttrs (
          name: type:
            type == "regular" && lib.hasSuffix ".ncl" name
        )
        entries;
    in
      lib.mapAttrs' (
        name: _: {
          name = lib.removeSuffix ".ncl" name;
          value = dirPath + "/${name}";
        }
      )
      nclEntries
    else {};

  # Check if a directory exists within the workspace root
  dirExists = workspaceRoot: relativeDir:
    builtins.pathExists (workspaceRoot + "/${relativeDir}");

  # Discover all convention directories and their .ncl files.
  #
  # Returns:
  # {
  #   packages = { hello = /path/to/packages/hello.ncl; ... };
  #   shells = { default = /path/to/shells/default.ncl; ... };
  #   ...
  # }
  #
  discoverAll = workspaceRoot: conventionOverrides: let
    conventions = applyConventionOverrides defaultConventions (
      if conventionOverrides == null
      then {}
      else conventionOverrides
    );
    activeConventions = lib.filterAttrs (_: conv: conv.autoDiscover or true) conventions;
  in
    lib.mapAttrs (
      _name: conv:
        discoverNclFiles workspaceRoot conv.dir
    )
    activeConventions;

  # Discover subworkspaces: subdirectories that contain a workspace.ncl file.
  #
  # Returns a list of { name, path } records.
  #
  discoverSubworkspaces = workspaceRoot: let
    entries = builtins.readDir workspaceRoot;
    dirs = lib.filterAttrs (_: type: type == "directory") entries;
  in
    lib.filterAttrs (
      name: _:
        builtins.pathExists (workspaceRoot + "/${name}/workspace.ncl")
    ) (
      lib.mapAttrs (
        name: _: workspaceRoot + "/${name}"
      )
      dirs
    );

  # Generate a mapping of discovered output names.
  #
  # Resolves "default" entries for subworkspaces:
  #   In a subworkspace named "foo", packages/default.ncl → "foo"
  #   Named entries get prefixed: packages/bar.ncl → "foo-bar"
  #
  resolveNames = {
    workspaceName ? null,
    isSubworkspace ? false,
  }: discovered:
    lib.mapAttrs (
      _conventionName: files:
        lib.listToAttrs (
          lib.mapAttrsToList (
            fileName: filePath: let
              outputName =
                if isSubworkspace && workspaceName != null
                then
                  if fileName == "default"
                  then workspaceName
                  else "${workspaceName}-${fileName}"
                else fileName;
            in {
              name = outputName;
              value = filePath;
            }
          )
          files
        )
    )
    discovered;
in {
  inherit
    defaultConventions
    applyConventionOverrides
    discoverNclFiles
    discoverAll
    discoverSubworkspaces
    resolveNames
    dirExists
    ;
}
