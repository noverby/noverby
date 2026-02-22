# eval-nickel.nix — Evaluate workspace.ncl via Nickel and import the result as JSON
#
# Uses Import From Derivation (IFD) to bridge Nickel → Nix:
#   1. Generate a wrapper .ncl that imports discovered files and applies contracts
#   2. Run `nickel export` to produce JSON
#   3. Read the JSON back into Nix
#
# This module is the bridge between the Nickel validation layer and the Nix
# builder layer. All contract checking happens inside the Nickel evaluation;
# the Nix side receives a fully validated configuration tree.
#
# Updated for v0.3 to support subworkspace evaluation. Each subworkspace
# gets its own Nickel evaluation pass, producing an independent validated
# config tree that is later merged with namespacing by the Nix layer.
{lib}: let
  # Generate the Nickel source for a single discovered entry.
  # Each entry becomes a field in the discovered record.
  #   e.g. { hello = import "/nix/store/.../packages/hello.ncl" }
  mkImportField = name: path: ''"${name}" = import "${toString path}",'';

  # Generate a block of Nickel import fields for a convention directory.
  mkImportBlock = entries:
    lib.concatStringsSep "\n" (lib.mapAttrsToList mkImportField entries);

  # Build the wrapper .ncl source that ties everything together:
  #   - Imports the nix-workspace contracts
  #   - Imports discovered .ncl files from convention directories
  #   - Imports the user's workspace.ncl (if present)
  #   - Merges discovered config with workspace config
  #   - Applies the WorkspaceConfig contract
  generateWrapperSource = {
    contractsDir,
    workspaceRoot,
    discoveredPackages ? {},
    discoveredShells ? {},
    discoveredMachines ? {},
    discoveredModules ? {},
    discoveredHome ? {},
    hasWorkspaceNcl ? false,
  }: let
    packageFields = mkImportBlock discoveredPackages;
    shellFields = mkImportBlock discoveredShells;
    machineFields = mkImportBlock discoveredMachines;
    moduleFields = mkImportBlock discoveredModules;
    homeFields = mkImportBlock discoveredHome;

    # If workspace.ncl exists, import and merge it; otherwise use empty record
    workspaceMerge =
      if hasWorkspaceNcl
      then ''
        let workspace_config = import "${toString workspaceRoot}/workspace.ncl" in
        (discovered & workspace_config)
      ''
      else "discovered";
  in ''
    let { WorkspaceConfig, .. } = import "${toString contractsDir}/workspace.ncl" in
    let discovered = {
      packages = {
    ${packageFields}
      },
      shells = {
    ${shellFields}
      },
      machines = {
    ${machineFields}
      },
      modules = {
    ${moduleFields}
      },
      home = {
    ${homeFields}
      },
    } in
    (${lib.strings.trim workspaceMerge}) | WorkspaceConfig
  '';

  # Generate a wrapper .ncl source for a subworkspace.
  #
  # This is similar to generateWrapperSource but tailored for subworkspaces:
  #   - Uses the subworkspace's own workspace.ncl
  #   - Discovers from the subworkspace's convention directories
  #   - Applies the same WorkspaceConfig contract
  #
  # The result is an independent config tree. Namespacing is applied
  # on the Nix side after evaluation.
  generateSubworkspaceWrapperSource = {
    contractsDir,
    subworkspaceRoot,
    subworkspaceName,
    discoveredPackages ? {},
    discoveredShells ? {},
    discoveredMachines ? {},
    discoveredModules ? {},
    discoveredHome ? {},
    hasWorkspaceNcl ? true,
  }: let
    packageFields = mkImportBlock discoveredPackages;
    shellFields = mkImportBlock discoveredShells;
    machineFields = mkImportBlock discoveredMachines;
    moduleFields = mkImportBlock discoveredModules;
    homeFields = mkImportBlock discoveredHome;

    workspaceMerge =
      if hasWorkspaceNcl
      then ''
        let workspace_config = import "${toString subworkspaceRoot}/workspace.ncl" in
        (discovered & workspace_config)
      ''
      else ''
        (discovered & { name = "${subworkspaceName}" })
      '';
  in ''
    let { WorkspaceConfig, .. } = import "${toString contractsDir}/workspace.ncl" in
    let discovered = {
      packages = {
    ${packageFields}
      },
      shells = {
    ${shellFields}
      },
      machines = {
    ${machineFields}
      },
      modules = {
    ${moduleFields}
      },
      home = {
    ${homeFields}
      },
    } in
    (${lib.strings.trim workspaceMerge}) | WorkspaceConfig
  '';

  # Evaluate a workspace by running Nickel and reading back JSON via IFD.
  #
  # Arguments:
  #   bootstrapPkgs      — A nixpkgs package set used to obtain the `nickel` binary
  #   contractsDir       — Path to the nix-workspace contracts/ directory
  #   workspaceRoot      — Path to the user's workspace root
  #   discoveredPackages — Attrset of { name = /path/to/name.ncl; ... }
  #   discoveredShells   — Attrset of { name = /path/to/name.ncl; ... }
  #   discoveredMachines — Attrset of { name = /path/to/name.ncl; ... }
  #   discoveredModules  — Attrset of { name = /path/to/name.ncl; ... }
  #   discoveredHome     — Attrset of { name = /path/to/name.ncl; ... }
  #
  # Returns: An attribute set (the validated workspace configuration).
  evalWorkspace = {
    bootstrapPkgs,
    contractsDir,
    workspaceRoot,
    discoveredPackages ? {},
    discoveredShells ? {},
    discoveredMachines ? {},
    discoveredModules ? {},
    discoveredHome ? {},
  }: let
    hasWorkspaceNcl = builtins.pathExists (workspaceRoot + "/workspace.ncl");

    wrapperSource = generateWrapperSource {
      inherit
        contractsDir
        workspaceRoot
        discoveredPackages
        discoveredShells
        discoveredMachines
        discoveredModules
        discoveredHome
        hasWorkspaceNcl
        ;
    };

    # Write the generated wrapper to the store so Nickel can import from it.
    # writeTextFile properly tracks store-path references in the text,
    # ensuring the sandbox has access to contracts and workspace sources.
    wrapperFile = bootstrapPkgs.writeTextFile {
      name = "nix-workspace-eval.ncl";
      text = wrapperSource;
    };

    # Run nickel export inside a derivation (IFD).
    # The output is a single JSON file representing the validated config.
    evalDrv =
      bootstrapPkgs.runCommand "nix-workspace-eval" {
        nativeBuildInputs = [bootstrapPkgs.nickel];

        # Explicitly reference source paths so they appear in the build sandbox.
        # Even though wrapperFile already references them textually, being
        # explicit avoids any edge-case sandbox issues.
        inherit contractsDir workspaceRoot;
      } ''
        nickel export ${wrapperFile} > $out
      '';
  in
    builtins.fromJSON (builtins.readFile evalDrv);

  # Evaluate a subworkspace's workspace.ncl through Nickel.
  #
  # This is the subworkspace counterpart of evalWorkspace. It produces
  # an independent validated config tree for a single subworkspace.
  # The caller is responsible for namespacing the outputs after evaluation.
  #
  # Arguments:
  #   bootstrapPkgs      — A nixpkgs package set (for nickel binary)
  #   contractsDir       — Path to nix-workspace contracts/
  #   subworkspaceRoot   — Absolute path to the subworkspace directory
  #   subworkspaceName   — Directory name of the subworkspace (e.g. "mojo-zed")
  #   discoveredPackages — { name = path; ... } from subworkspace discovery
  #   discoveredShells   — { name = path; ... }
  #   discoveredMachines — { name = path; ... }
  #   discoveredModules  — { name = path; ... }
  #   discoveredHome     — { name = path; ... }
  #
  # Returns: An attribute set (the validated subworkspace configuration).
  evalSubworkspace = {
    bootstrapPkgs,
    contractsDir,
    subworkspaceRoot,
    subworkspaceName,
    discoveredPackages ? {},
    discoveredShells ? {},
    discoveredMachines ? {},
    discoveredModules ? {},
    discoveredHome ? {},
  }: let
    hasWorkspaceNcl = builtins.pathExists (subworkspaceRoot + "/workspace.ncl");

    wrapperSource = generateSubworkspaceWrapperSource {
      inherit
        contractsDir
        subworkspaceRoot
        subworkspaceName
        discoveredPackages
        discoveredShells
        discoveredMachines
        discoveredModules
        discoveredHome
        hasWorkspaceNcl
        ;
    };

    wrapperFile = bootstrapPkgs.writeTextFile {
      name = "nix-workspace-eval-${subworkspaceName}.ncl";
      text = wrapperSource;
    };

    evalDrv =
      bootstrapPkgs.runCommand "nix-workspace-eval-${subworkspaceName}" {
        nativeBuildInputs = [bootstrapPkgs.nickel];
        inherit contractsDir subworkspaceRoot;
      } ''
        nickel export ${wrapperFile} > $out
      '';
  in
    builtins.fromJSON (builtins.readFile evalDrv);

  # Evaluate all subworkspaces discovered in a workspace.
  #
  # Type: AttrSet -> AttrSet
  #
  # Arguments:
  #   bootstrapPkgs    — A nixpkgs package set (for nickel binary)
  #   contractsDir     — Path to nix-workspace contracts/
  #   subworkspaceMap  — Output of discover.discoverAllSubworkspaces
  #                      { name = { path, discovered, hasWorkspaceNcl }; ... }
  #
  # Returns:
  #   { name = evaluatedConfig; ... }
  #   where each evaluatedConfig is the full validated workspace config
  #   for that subworkspace.
  evalAllSubworkspaces = {
    bootstrapPkgs,
    contractsDir,
    subworkspaceMap,
  }:
    lib.mapAttrs (
      name: info: let
        inherit (info) discovered;
      in
        evalSubworkspace {
          inherit bootstrapPkgs contractsDir;
          subworkspaceRoot = info.path;
          subworkspaceName = name;
          discoveredPackages = discovered.packages or {};
          discoveredShells = discovered.shells or {};
          discoveredMachines = discovered.machines or {};
          discoveredModules = discovered.modules or {};
          discoveredHome = discovered.home or {};
        }
    )
    subworkspaceMap;

  # Light-weight evaluation: skip Nickel entirely and return a minimal
  # default config. Used as a fallback when there is no workspace.ncl
  # and no discovered .ncl files, so we can still produce outputs from
  # the Nix-side config alone.
  emptyConfig = {
    name = "workspace";
    systems = ["x86_64-linux" "aarch64-linux"];
    nixpkgs = {};
    packages = {};
    shells = {};
    machines = {};
    modules = {};
    home = {};
    conventions = {};
    dependencies = {};
  };
in {
  inherit
    evalWorkspace
    evalSubworkspace
    evalAllSubworkspaces
    generateWrapperSource
    generateSubworkspaceWrapperSource
    emptyConfig
    ;
}
