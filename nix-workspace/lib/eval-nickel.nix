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
    hasWorkspaceNcl ? false,
  }: let
    packageFields = mkImportBlock discoveredPackages;
    shellFields = mkImportBlock discoveredShells;

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
    } in
    (${lib.strings.trim workspaceMerge}) | WorkspaceConfig
  '';

  # Evaluate a workspace by running Nickel and reading back JSON via IFD.
  #
  # Arguments:
  #   bootstrapPkgs   — A nixpkgs package set used to obtain the `nickel` binary
  #   contractsDir    — Path to the nix-workspace contracts/ directory
  #   workspaceRoot   — Path to the user's workspace root
  #   discoveredPackages — Attrset of { name = /path/to/name.ncl; ... }
  #   discoveredShells   — Attrset of { name = /path/to/name.ncl; ... }
  #
  # Returns: An attribute set (the validated workspace configuration).
  evalWorkspace = {
    bootstrapPkgs,
    contractsDir,
    workspaceRoot,
    discoveredPackages ? {},
    discoveredShells ? {},
  }: let
    hasWorkspaceNcl = builtins.pathExists (workspaceRoot + "/workspace.ncl");

    wrapperSource = generateWrapperSource {
      inherit contractsDir workspaceRoot discoveredPackages discoveredShells hasWorkspaceNcl;
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
    conventions = {};
  };
in {
  inherit evalWorkspace generateWrapperSource emptyConfig;
}
